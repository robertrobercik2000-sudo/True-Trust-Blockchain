#![forbid(unsafe_code)]

//! Pełny STARK range-proof dla transakcji (Winterfell 0.13)
//!
//! Używany przez `tx_stark.rs` via:
//!   use crate::stark_full::{STARKProver, STARKVerifier};
//!
//! API:
//!   - STARKProver::prove_range_with_commitment(value, &commitment) -> StarkRangeProof
//!   - STARKVerifier::verify(&StarkRangeProof) -> bool
//!
//! Zakres: dowód, że 0 <= value < 2^num_bits (tu: 64 bity).
//!
//! Uwaga: samo AIR dowodzi tylko zakresu. Powiązanie z commitmentem
//! (c = H("TX_OUTPUT_STARK.v1" || value || blinding || recipient))
//! jest robione na poziomie protokołu (zob. tx_stark.rs, decrypt_and_verify).

use serde::{Deserialize, Serialize};
use crate::core::Hash32;

use winterfell::{
    Air, AirContext, Assertion, EvaluationFrame, Proof, ProofOptions, Prover, Trace, TraceInfo,
    TransitionConstraintDegree, verify, AcceptableOptions,
    crypto::{hashers::Blake3_256, DefaultRandomCoin, MerkleTree},
    math::{fields::f128::BaseElement, FieldElement, ToElements},
    matrix::ColMatrix,
    CompositionPoly, CompositionPolyTrace, DefaultConstraintCommitment, DefaultConstraintEvaluator,
    DefaultTraceLde, FieldExtension, PartitionOptions, StarkDomain, TracePolyTable, TraceTable,
    BatchingMethod,
};

// StarkField trait daje as_int() dla BaseElement
use winterfell::math::StarkField;

/// Public inputs publikowane razem z dowodem.
///
/// W naszym przypadku:
/// - value: liczba u64, której zakres 0 <= value < 2^num_bits dowodzimy,
/// - num_bits: liczba bitów (typowo 64).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PublicInputs {
    /// Wynik: suma bitów = value (u64)
    pub value: u64,
    /// Liczba bitów użyta w śladzie (np. 64)
    pub num_bits: u32,
}

impl ToElements<BaseElement> for PublicInputs {
    fn to_elements(&self) -> Vec<BaseElement> {
        vec![
            BaseElement::from(self.value),
            BaseElement::from(self.num_bits as u64),
        ]
    }
}

/// Indeksy kolumn w trace
const COL_SUM: usize = 0;
const COL_BIT: usize = 1;
const COL_POW2: usize = 2;

/// AIR dla range proofa:
///
/// - kolumny: SUM, BIT, POW2
/// - przejścia:
///   * next_sum = sum + bit * pow2
///   * next_pow2 = pow2 + pow2
///   * bit * (bit - 1) = 0
/// - asercje:
///   * sum(0) = 0
///   * pow2(0) = 1
///   * sum(last) = value
pub struct RangeAir {
    context: AirContext<BaseElement>,
    result: BaseElement, // value
}

impl Air for RangeAir {
    type BaseField = BaseElement;
    type PublicInputs = PublicInputs;

    fn new(trace_info: TraceInfo, pub_inputs: PublicInputs, options: ProofOptions) -> Self {
        debug_assert_eq!(trace_info.width(), 3);

        // stopnie wielomianów dla constraintów:
        // - C1: next_sum - (sum + bit * pow2) → deg 2
        // - C2: next_pow2 - (pow2 + pow2)     → deg 1
        // - C3: bit * (bit - 1)               → deg 2
        let degrees = vec![
            TransitionConstraintDegree::new(2),
            TransitionConstraintDegree::new(1),
            TransitionConstraintDegree::new(2),
        ];
        let num_assertions = 3;

        Self {
            context: AirContext::new(trace_info, degrees, num_assertions, options),
            result: BaseElement::from(pub_inputs.value),
        }
    }

    fn evaluate_transition<E: FieldElement + From<Self::BaseField>>(
        &self,
        frame: &EvaluationFrame<E>,
        _periodic_values: &[E],
        result: &mut [E],
    ) {
        let cur = frame.current();
        let nxt = frame.next();

        let sum = cur[COL_SUM];
        let bit = cur[COL_BIT];
        let pow2 = cur[COL_POW2];

        let next_sum = nxt[COL_SUM];
        let next_pow2 = nxt[COL_POW2];

        // C1: next_sum - (sum + bit * pow2) = 0
        result[0] = next_sum - (sum + bit * pow2);
        // C2: next_pow2 - (pow2 + pow2) = 0
        result[1] = next_pow2 - (pow2 + pow2);
        // C3: bit*(bit-1) = 0 (bit ∈ {0,1})
        result[2] = bit * (bit - E::ONE);
    }

    fn get_assertions(&self) -> Vec<Assertion<Self::BaseField>> {
        let last = self.trace_length() - 1;
        vec![
            // sum(0) = 0
            Assertion::single(COL_SUM, 0, BaseElement::ZERO),
            // pow2(0) = 1
            Assertion::single(COL_POW2, 0, BaseElement::ONE),
            // sum(LAST) = value
            Assertion::single(COL_SUM, last, self.result),
        ]
    }

    fn context(&self) -> &AirContext<Self::BaseField> {
        &self.context
    }
}

/// Prover STARK range-proofa
pub struct RangeProver {
    options: ProofOptions,
    num_bits: usize,
}

impl RangeProver {
    pub fn new(num_bits: usize, options: ProofOptions) -> Self {
        Self { options, num_bits }
    }

    /// Buduje trace dla value (u64):
    ///
    /// - liczba wierszy: potęga 2 >= num_bits + 1 (np. 128 dla 64 bitów)
    /// - kolumny: SUM / BIT / POW2
    pub fn build_trace(&self, value: u64) -> TraceTable<BaseElement> {
        let required = self.num_bits + 1;
        // Round up to next power of 2
        let n = required.next_power_of_two();

        let mut sum_col = vec![BaseElement::ZERO; n];
        let mut bit_col = vec![BaseElement::ZERO; n];
        let mut pow2_col = vec![BaseElement::ZERO; n];

        sum_col[0] = BaseElement::ZERO;
        pow2_col[0] = BaseElement::ONE;

        // Rozkład na bity
        for i in 0..self.num_bits {
            let b = ((value >> i) & 1) as u64;
            bit_col[i] = BaseElement::from(b);

            // next_sum = sum + bit * pow2
            let next_sum = sum_col[i] + bit_col[i] * pow2_col[i];
            sum_col[i + 1] = next_sum;

            // next_pow2 = pow2 + pow2 (czyli *2)
            pow2_col[i + 1] = pow2_col[i] + pow2_col[i];
        }

        // Pad remaining rows to maintain constraints
        for i in (self.num_bits + 1)..n {
            sum_col[i] = sum_col[i - 1];
            bit_col[i] = BaseElement::ZERO;
            pow2_col[i] = pow2_col[i - 1] + pow2_col[i - 1];
        }

        TraceTable::init(vec![sum_col, bit_col, pow2_col])
    }
}

impl Prover for RangeProver {
    type BaseField = BaseElement;
    type Air = RangeAir;
    type Trace = TraceTable<Self::BaseField>;
    type HashFn = Blake3_256<Self::BaseField>;
    type VC = MerkleTree<Self::HashFn>;
    type RandomCoin = DefaultRandomCoin<Self::HashFn>;
    type TraceLde<E: FieldElement<BaseField = Self::BaseField>> =
        DefaultTraceLde<E, Self::HashFn, Self::VC>;
    type ConstraintCommitment<E: FieldElement<BaseField = Self::BaseField>> =
        DefaultConstraintCommitment<E, Self::HashFn, Self::VC>;
    type ConstraintEvaluator<'a, E: FieldElement<BaseField = Self::BaseField>> =
        DefaultConstraintEvaluator<'a, Self::Air, E>;

    fn get_pub_inputs(&self, trace: &Self::Trace) -> PublicInputs {
        let last = trace.length() - 1;
        let elem = trace.get(COL_SUM, last);
        // BaseElement → u64
        let value = elem.as_int() as u64;
        PublicInputs {
            value,
            num_bits: self.num_bits as u32,
        }
    }

    fn options(&self) -> &ProofOptions {
        &self.options
    }

    fn new_trace_lde<E: FieldElement<BaseField = Self::BaseField>>(
        &self,
        trace_info: &TraceInfo,
        main_trace: &ColMatrix<Self::BaseField>,
        domain: &StarkDomain<Self::BaseField>,
        partition_option: PartitionOptions,
    ) -> (Self::TraceLde<E>, TracePolyTable<E>) {
        DefaultTraceLde::new(trace_info, main_trace, domain, partition_option)
    }

    fn build_constraint_commitment<E: FieldElement<BaseField = Self::BaseField>>(
        &self,
        composition_poly_trace: CompositionPolyTrace<E>,
        num_constraint_composition_columns: usize,
        domain: &StarkDomain<Self::BaseField>,
        partition_options: PartitionOptions,
    ) -> (Self::ConstraintCommitment<E>, CompositionPoly<E>) {
        DefaultConstraintCommitment::new(
            composition_poly_trace,
            num_constraint_composition_columns,
            domain,
            partition_options,
        )
    }

    fn new_evaluator<'a, E: FieldElement<BaseField = Self::BaseField>>(
        &self,
        air: &'a Self::Air,
        aux_rand_elements: Option<winterfell::AuxRandElements<E>>,
        composition_coefficients: winterfell::ConstraintCompositionCoefficients<E>,
    ) -> Self::ConstraintEvaluator<'a, E> {
        DefaultConstraintEvaluator::new(air, aux_rand_elements, composition_coefficients)
    }
}

/// Domyślne ProofOptions ~95–100 bitów bezpieczeństwa
pub fn default_proof_options() -> ProofOptions {
    ProofOptions::new(
        32,                    // liczba zapytań
        8,                     // blowup factor
        0,                     // grinding factor
        FieldExtension::None,  // brak rozszerzenia pola
        8,                     // FRI folding factor
        31,                    // FRI max remainder degree
        BatchingMethod::Linear,
        BatchingMethod::Linear,
    )
}

/// Niskopoziomowa funkcja: generuje dowód i public inputs
pub fn prove_range(value: u64, num_bits: usize, options: ProofOptions) -> (Proof, PublicInputs) {
    let prover = RangeProver::new(num_bits, options);
    let trace = prover.build_trace(value);
    let pub_inputs = prover.get_pub_inputs(&trace);
    let proof = prover.prove(trace).expect("STARK proof generation failed");
    (proof, pub_inputs)
}

/// Niskopoziomowa funkcja: weryfikacja Winterfell Proof + PublicInputs
pub fn verify_range(proof: Proof, pub_inputs: PublicInputs) -> bool {
    let acceptable = AcceptableOptions::MinConjecturedSecurity(95);
    verify::<RangeAir, Blake3_256<BaseElement>, DefaultRandomCoin<Blake3_256<BaseElement>>, MerkleTree<Blake3_256<BaseElement>>>(
        proof,
        pub_inputs,
        &acceptable,
    )
    .is_ok()
}

/// PublicInputs → 12 bajtów (value LE 8B + num_bits LE 4B)
pub fn encode_public_inputs(pi: &PublicInputs) -> [u8; 12] {
    let mut out = [0u8; 12];
    out[0..8].copy_from_slice(&pi.value.to_le_bytes());
    out[8..12].copy_from_slice(&pi.num_bits.to_le_bytes());
    out
}

/// 12 bajtów → PublicInputs
pub fn decode_public_inputs(bytes: &[u8]) -> Option<PublicInputs> {
    if bytes.len() != 12 {
        return None;
    }
    let mut v8 = [0u8; 8];
    v8.copy_from_slice(&bytes[0..8]);
    let value = u64::from_le_bytes(v8);

    let mut b4 = [0u8; 4];
    b4.copy_from_slice(&bytes[8..12]);
    let num_bits = u32::from_le_bytes(b4);

    Some(PublicInputs { value, num_bits })
}

/// To jest typ, który będzie serializowany przez bincode
/// i używany w TxOutputStark::stark_proof.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StarkRangeProof {
    /// Surowe bajty Winterfell::Proof::to_bytes()
    pub proof_bytes: Vec<u8>,
    /// Public inputs (zawierają m.in. value)
    pub pub_inputs: PublicInputs,
}

/// Prover używany przez tx_stark.rs
pub struct STARKProver;

/// Verifier używany przez tx_stark.rs
pub struct STARKVerifier;

impl STARKProver {
    /// Główna funkcja, której oczekuje `tx_stark.rs`.
    ///
    /// `commitment` jest na razie tylko parametrem "dla API" – samo AIR
    /// dowodzi zakresu. Powiązanie value ↔ commitment robisz na poziomie
    /// protokołu (zob. TxOutputStark::decrypt_and_verify).
    pub fn prove_range_with_commitment(value: u64, _commitment: &Hash32) -> StarkRangeProof {
        let options = default_proof_options();
        // 64 bity zakresu (0..2^64)
        let (proof, pub_inputs) = prove_range(value, 64, options);
        let proof_bytes = proof.to_bytes();
        StarkRangeProof { proof_bytes, pub_inputs }
    }
}

impl STARKVerifier {
    /// Weryfikacja bazująca na tym co robi Twój TxOutputStark:
    ///
    /// ```rust
    /// if let Ok(proof) = bincode::deserialize(&self.stark_proof) {
    ///     STARKVerifier::verify(&proof)
    /// }
    /// ```
    pub fn verify(p: &StarkRangeProof) -> bool {
        let acceptable = AcceptableOptions::MinConjecturedSecurity(95);

        // Odtworzenie Winterfell::Proof z bajtów
        let proof = match Proof::from_bytes(&p.proof_bytes) {
            Ok(pr) => pr,
            Err(_) => return false,
        };

        verify::<RangeAir, Blake3_256<BaseElement>, DefaultRandomCoin<Blake3_256<BaseElement>>, MerkleTree<Blake3_256<BaseElement>>>(
            proof,
            p.pub_inputs,
            &acceptable,
        )
        .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_proof_roundtrip() {
        let value = 42u64;
        let opts = default_proof_options();
        let (proof, pi) = prove_range(value, 64, opts);
        assert_eq!(pi.value, value);
        assert!(verify_range(proof, pi));
    }

    #[test]
    fn range_proof_wrong_value_fails() {
        let value = 12345u64;
        let (proof, mut pi) = prove_range(value, 64, default_proof_options());
        pi.value = value + 1;
        assert!(!verify_range(proof, pi));
    }

    #[test]
    fn public_inputs_codec() {
        let pi = PublicInputs { value: 777, num_bits: 64 };
        let enc = encode_public_inputs(&pi);
        let dec = decode_public_inputs(&enc).unwrap();
        assert_eq!(pi.value, dec.value);
        assert_eq!(pi.num_bits, dec.num_bits);
    }

    #[test]
    fn api_compatible_with_tx_stark() {
        use rand::RngCore;
        use sha3::{Digest, Sha3_256};

        // symulacja commitmentu jak w TxOutputStark::new
        let mut blinding = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut blinding);
        let recipient = [1u8; 32];
        let value = 123u64;

        let mut h = Sha3_256::new();
        h.update(b"TX_OUTPUT_STARK.v1");
        h.update(&value.to_le_bytes());
        h.update(&blinding);
        h.update(&recipient);
        let commitment: Hash32 = h.finalize().into();

        // Prover
        let proof = STARKProver::prove_range_with_commitment(value, &commitment);
        let encoded = bincode::serialize(&proof).unwrap();

        // Verifier (tak jak w TxOutputStark::verify)
        let decoded: StarkRangeProof = bincode::deserialize(&encoded).unwrap();
        assert!(STARKVerifier::verify(&decoded));
    }
}
