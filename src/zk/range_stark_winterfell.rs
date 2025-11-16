#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use winterfell::{
    crypto::{hashers::Blake3_256, DefaultRandomCoin, MerkleTree},
    math::{fields::f128::BaseElement, FieldElement},
    matrix::ColMatrix,
    Air, AirContext, Assertion, EvaluationFrame, Proof, ProofOptions, Prover,
    StarkDomain, Trace, TraceInfo, TracePolyTable, TraceTable,
    TransitionConstraintDegree,
    AcceptableOptions, BatchingMethod, CompositionPoly, CompositionPolyTrace,
    DefaultConstraintCommitment, DefaultConstraintEvaluator, DefaultTraceLde,
    FieldExtension, PartitionOptions,
};

/// Public inputs (start value + 256-bit commitment)
#[derive(Clone, Copy)]
pub struct PublicInputs {
    start: BaseElement,
    commitment: [BaseElement; 4],
}

impl winterfell::ToElements<BaseElement> for PublicInputs {
    fn to_elements(&self) -> Vec<BaseElement> {
        let mut v = Vec::with_capacity(5);
        v.push(self.start);
        v.extend_from_slice(&self.commitment);
        v
    }
}

/// AIR: 2 kolumny: remainder, bit.
/// Przejście: 2*rem(next) + bit(cur) - rem(cur) = 0; Booleanity: bit∈{0,1}.
pub struct RangeAir {
    ctx: AirContext<BaseElement>,
    start: BaseElement,
}

impl Air for RangeAir {
    type BaseField = BaseElement;
    type PublicInputs = PublicInputs;

    fn new(trace_info: TraceInfo, pub_inputs: PublicInputs, options: ProofOptions) -> Self {
        assert_eq!(2, trace_info.width());
        let degrees = vec![TransitionConstraintDegree::new(1), TransitionConstraintDegree::new(2)];
        let num_assertions = 2;
        Self { ctx: AirContext::new(trace_info, degrees, num_assertions, options), start: pub_inputs.start }
    }

    fn evaluate_transition<E: FieldElement + From<Self::BaseField>>(
        &self,
        frame: &EvaluationFrame<E>,
        _periodic: &[E],
        result: &mut [E],
    ) {
        let cur = frame.current();
        let nxt = frame.next();
        let two = E::from(2u8);
        let one = E::from(1u8);
        result[0] = (nxt[0] * two) + cur[1] - cur[0];
        result[1] = cur[1] * (cur[1] - one);
    }

    fn get_assertions(&self) -> Vec<Assertion<Self::BaseField>> {
        let last = self.trace_length() - 1;
        vec![
            Assertion::single(0, 0, self.start),
            Assertion::single(0, last, BaseElement::ZERO),
        ]
    }

    fn context(&self) -> &AirContext<Self::BaseField> { &self.ctx }
}

/// Prover range+binding.
pub struct RangeProver {
    opts: ProofOptions,
    commitment_fe: [BaseElement; 4],
}

impl RangeProver {
    pub fn new(opts: ProofOptions, commitment: [u8; 32]) -> Self {
        Self { opts, commitment_fe: commitment_to_elements(commitment) }
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
        let start = trace.get(0, 0);
        PublicInputs { start, commitment: self.commitment_fe }
    }

    fn options(&self) -> &ProofOptions { &self.opts }

    fn new_trace_lde<E: FieldElement<BaseField = Self::BaseField>>(
        &self,
        trace_info: &TraceInfo,
        main_trace: &ColMatrix<Self::BaseField>,
        domain: &StarkDomain<Self::BaseField>,
        partition: PartitionOptions,
    ) -> (Self::TraceLde<E>, TracePolyTable<E>) {
        DefaultTraceLde::new(trace_info, main_trace, domain, partition)
    }

    fn build_constraint_commitment<E: FieldElement<BaseField = Self::BaseField>>(
        &self,
        composition_poly_trace: CompositionPolyTrace<E>,
        num_cols: usize,
        domain: &StarkDomain<Self::BaseField>,
        partition: PartitionOptions,
    ) -> (Self::ConstraintCommitment<E>, CompositionPoly<E>) {
        DefaultConstraintCommitment::new(composition_poly_trace, num_cols, domain, partition)
    }

    fn new_evaluator<'a, E: FieldElement<BaseField = Self::BaseField>>(
        &self,
        air: &'a Self::Air,
        aux_rand_elements: Option<winterfell::AuxRandElements<E>>,
        coeffs: winterfell::ConstraintCompositionCoefficients<E>,
    ) -> Self::ConstraintEvaluator<'a, E> {
        DefaultConstraintEvaluator::new(air, aux_rand_elements, coeffs)
    }
}

/// Uproszczone, deterministyczne parametry proof.
fn default_proof_options() -> ProofOptions {
    ProofOptions::new(
        32,                       // queries
        8,                        // blowup
        0,                        // grinding
        FieldExtension::None,
        8,                        // FRI folding
        31,                       // max remainder deg
        BatchingMethod::Linear,
        BatchingMethod::Linear,
    )
}

/// Ślad: 64 kroki → 65 wierszy.
fn build_trace(value: u64, bits: usize) -> TraceTable<BaseElement> {
    let len = bits + 1;
    let mut rems = Vec::with_capacity(len);
    let mut bits_v = Vec::with_capacity(len);
    let mut r: u128 = value as u128;
    for _ in 0..bits {
        let b = (r & 1) as u64;
        rems.push(BaseElement::from(r as u64));
        bits_v.push(BaseElement::from(b));
        r = (r - (b as u128)) >> 1;
    }
    rems.push(BaseElement::from(r as u64));
    bits_v.push(BaseElement::ZERO);

    let mut trace = TraceTable::new(2, len);
    trace.fill(
        |state| { state[0] = rems[0]; state[1] = bits_v[0]; },
        |i, state| {
            let next = i + 1;
            state[0] = rems[next];
            state[1] = bits_v[next];
        },
    );
    trace
}

fn commitment_to_elements(c: [u8; 32]) -> [BaseElement; 4] {
    let mut out = [BaseElement::ZERO; 4];
    for i in 0..4 {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&c[8 * i..8 * (i + 1)]);
        out[i] = BaseElement::from(u64::from_le_bytes(buf));
    }
    out
}

/// Web-bezpieczne opakowanie proofu (public inputs + proof bytes).
#[derive(Serialize, Deserialize)]
struct WrappedProof {
    start_value: u64,
    commitment: [u8; 32],
    proof: ProofBytes,
}

#[derive(Serialize, Deserialize)]
struct ProofBytes(Vec<u8>);

impl From<Proof> for ProofBytes {
    fn from(p: Proof) -> Self { ProofBytes(p.to_bytes()) }
}
impl TryFrom<&ProofBytes> for Proof {
    type Error = winterfell::ProverError;
    fn try_from(pb: &ProofBytes) -> Result<Self, Self::Error> { Proof::from_bytes(&pb.0) }
}

/// API: generacja proofu (zawiera inputs).
pub fn prove_range_bound(value: u64, commitment: [u8; 32], opts: Option<ProofOptions>) -> Vec<u8> {
    let trace = build_trace(value, 64);
    let prover = RangeProver::new(opts.unwrap_or_else(default_proof_options), commitment);
    let proof = prover.prove(trace).expect("proof generation failed");
    let wrapped = WrappedProof { start_value: value, commitment, proof: proof.into() };
    bincode::serialize(&wrapped).expect("serialize proof failed")
}

/// API: weryfikacja proofu i wiązania commitmentu.
pub fn verify_range_bound_with_commitment(bytes: &[u8], expected_commitment: [u8; 32]) -> bool {
    let wrapped: WrappedProof = match bincode::deserialize(bytes) { Ok(w) => w, Err(_) => return false };
    if wrapped.commitment != expected_commitment { return false; }

    let pub_inputs = PublicInputs {
        start: BaseElement::from(wrapped.start_value),
        commitment: commitment_to_elements(wrapped.commitment),
    };
    let acceptable = AcceptableOptions::MinConjecturedSecurity(95);
    let proof = match Proof::try_from(&wrapped.proof) { Ok(p) => p, Err(_) => return false };

    winterfell::verify::<
        RangeAir,
        Blake3_256<BaseElement>,
        DefaultRandomCoin<Blake3_256<BaseElement>>,
        MerkleTree<Blake3_256<BaseElement>>,
    >(proof, pub_inputs, &acceptable).is_ok()
}

// pomocnicze: hash pola dla ewentualnego testu ścieżek
fn _hash_field(elem: BaseElement) -> [u8; 32] {
    let mut h = Sha3_256::new();
    h.update(b"FIELD");
    h.update(&elem.as_int().to_le_bytes());
    h.finalize().into()
}
