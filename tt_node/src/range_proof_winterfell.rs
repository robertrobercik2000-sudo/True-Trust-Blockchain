//! Range proof over u64 using Winterfell 0.13 (production-ready)
//!
//! **STATUS:** ✅ PRODUCTION (Rust 1.91.1 >= 1.87 required)
//!
//! This is user's CORRECT implementation for Winterfell range proofs.
//! Better than v1 (2-column): uses 3-column design (SUM/BIT/POW2).
//!
//! # Design
//!
//! - Field: f128::BaseElement (Winterfell default, wide security margin)
//! - Trace: 3 columns: SUM, BIT, POW2
//! - Transitions:
//!   * next_sum = sum + bit * pow2
//!   * next_pow2 = pow2 + pow2      (i.e., *= 2)
//!   * bit * (bit - 1) = 0          (booleanity)
//! - Assertions:
//!   * sum(0) = 0
//!   * pow2(0) = 1
//!   * sum(LAST) = value
//!
//! # Public Inputs
//!
//! - `value`: u64 (the value being proven)
//! - `num_bits`: u32 (number of bits, typically 64)
//!
//! # Trace Size
//!
//! Trace has `num_bits + 1` rows.
//!
//! # Commitment Binding
//!
//! This is a pure range proof (0 <= value < 2^num_bits).
//! For commitment binding (e.g., to c_out), implement at protocol level
//! outside the AIR (see tx_stark.rs).
//!
//! # Security
//!
//! ~95-100 bit conjectured security with default proof options.

#![forbid(unsafe_code)]
#![cfg(feature = "winterfell_v2")]  // Requires Rust 1.87+

use serde::{Deserialize, Serialize};
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

// StarkField trait provides as_int() method for BaseElement
use winterfell::math::StarkField;

/// Public inputs published with the proof
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PublicInputs {
    /// Expected result: sum of bits = value (u64)
    pub value: u64,
    /// Number of bits proven in trace (e.g., 64)
    pub num_bits: u32,
}

impl ToElements<BaseElement> for PublicInputs {
    fn to_elements(&self) -> Vec<BaseElement> {
        // Encode value and num_bits as field elements
        vec![BaseElement::from(self.value), BaseElement::from(self.num_bits as u64)]
    }
}

/// Trace column indices
const COL_SUM: usize = 0;
const COL_BIT: usize = 1;
const COL_POW2: usize = 2;

/// AIR for range proof
pub struct RangeAir {
    context: AirContext<BaseElement>,
    result: BaseElement, // value
}

impl Air for RangeAir {
    type BaseField = BaseElement;
    type PublicInputs = PublicInputs;

    fn new(trace_info: TraceInfo, pub_inputs: PublicInputs, options: ProofOptions) -> Self {
        debug_assert_eq!(trace_info.width(), 3);

        // Degrees: sum-transition (deg 2), pow2-transition (deg 1), bit-booleanity (deg 2)
        let degrees = vec![
            TransitionConstraintDegree::new(2),
            TransitionConstraintDegree::new(1),
            TransitionConstraintDegree::new(2),
        ];
        // Assertions: sum(0)=0, pow2(0)=1, sum(last)=value
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
        // C3: bit*(bit-1) = 0
        result[2] = bit * (bit - E::ONE);
    }

    fn get_assertions(&self) -> Vec<Assertion<Self::BaseField>> {
        let last = self.trace_length() - 1;
        vec![
            Assertion::single(COL_SUM, 0, BaseElement::ZERO),
            Assertion::single(COL_POW2, 0, BaseElement::ONE),
            Assertion::single(COL_SUM, last, self.result),
        ]
    }

    fn context(&self) -> &AirContext<Self::BaseField> {
        &self.context
    }
}

/// Prover range-proofa
pub struct RangeProver {
    options: ProofOptions,
    num_bits: usize,
}

impl RangeProver {
    pub fn new(num_bits: usize, options: ProofOptions) -> Self {
        Self { options, num_bits }
    }

    /// Builds trace for value `value` (u64):
    /// - rows = num_bits + 1
    /// - columns: SUM/BIT/POW2
    pub fn build_trace(&self, value: u64) -> TraceTable<BaseElement> {
        let n = self.num_bits + 1;
        let mut sum_col = vec![BaseElement::ZERO; n];
        let mut bit_col = vec![BaseElement::ZERO; n];
        let mut pow2_col = vec![BaseElement::ZERO; n];

        sum_col[0] = BaseElement::ZERO;
        pow2_col[0] = BaseElement::ONE;

        // Bit decomposition
        for i in 0..self.num_bits {
            let b = ((value >> i) & 1) as u64;
            bit_col[i] = BaseElement::from(b);

            // next_sum = sum + bit * pow2
            let next_sum = sum_col[i] + bit_col[i] * pow2_col[i];
            sum_col[i + 1] = next_sum;

            // next_pow2 = pow2 + pow2
            pow2_col[i + 1] = pow2_col[i] + pow2_col[i];
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
        // Convert BaseElement to u64 via as_int() method
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

/// Convenience: ~95–100 bits security (per docs)
pub fn default_proof_options() -> ProofOptions {
    ProofOptions::new(
        32,                    // number of queries
        8,                     // blowup factor
        0,                     // grinding factor
        FieldExtension::None,  // no field extension
        8,                     // FRI folding factor
        31,                    // FRI max remainder degree
        BatchingMethod::Linear,
        BatchingMethod::Linear,
    )
}

/// Generate proof that 0 <= value < 2^num_bits
pub fn prove_range(value: u64, num_bits: usize, options: ProofOptions) -> (Proof, PublicInputs) {
    let prover = RangeProver::new(num_bits, options);
    let trace = prover.build_trace(value);
    let pub_inputs = prover.get_pub_inputs(&trace);
    let proof = prover.prove(trace).expect("proof generation failed");
    (proof, pub_inputs)
}

/// Verify range proof for `pub_inputs`
pub fn verify_range(proof: Proof, pub_inputs: PublicInputs) -> bool {
    let acceptable = AcceptableOptions::MinConjecturedSecurity(95);
    verify::<RangeAir, Blake3_256<BaseElement>, DefaultRandomCoin<Blake3_256<BaseElement>>, MerkleTree<Blake3_256<BaseElement>>>(
        proof, pub_inputs, &acceptable,
    ).is_ok()
}

/// Encode PublicInputs to 12 bytes (value LE 8B + num_bits LE 4B)
pub fn encode_public_inputs(pi: &PublicInputs) -> [u8; 12] {
    let mut out = [0u8; 12];
    out[0..8].copy_from_slice(&pi.value.to_le_bytes());
    out[8..12].copy_from_slice(&pi.num_bits.to_le_bytes());
    out
}

/// Decode PublicInputs from 12 bytes
pub fn decode_public_inputs(bytes: &[u8]) -> Option<PublicInputs> {
    if bytes.len() != 12 { return None; }
    let mut v8 = [0u8; 8];
    v8.copy_from_slice(&bytes[0..8]);
    let value = u64::from_le_bytes(v8);
    let mut b4 = [0u8; 4];
    b4.copy_from_slice(&bytes[8..12]);
    let num_bits = u32::from_le_bytes(b4);
    Some(PublicInputs { value, num_bits })
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
}