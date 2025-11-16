#![forbid(unsafe_code)]

//! STARK – szkic edukacyjny (NIE production)
//!
//! **Based on:**
//! - StarkWare's STARK protocol
//! - Polygon Miden VM architecture
//! - FRI (Fast Reed-Solomon Interactive Oracle Proof)
//!
//! **Components:**
//! 1. Prime Field Arithmetic (GF(p) = 2^31 - 2^27 + 1 – BabyBear)
//! 2. Polynomial Operations (evaluation, interpolation)
//! 3. AIR (Algebraic Intermediate Representation)
//! 4. FRI Protocol (folding + commitment)
//! 5. Merkle Trees (SHA-3 based)
//! 6. Fiat-Shamir (non-interactive)
//!
//! Ten kod nie zapewnia gwarancji wydajności ani poziomu bezpieczeństwa.

use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use std::ops::{Add, Mul, Sub};

pub type Hash32 = [u8; 32];

// ============================================================================
// PRIME FIELD ARITHMETIC
// ============================================================================

pub const FIELD_MODULUS: u64 = 2013265921; // 2^31 - 2^27 + 1
pub const MAX_2_ADIC_ORDER: usize = 27;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldElement(u64);

impl FieldElement {
    pub const ZERO: Self = FieldElement(0);
    pub const ONE: Self = FieldElement(1);

    pub fn new(val: u64) -> Self {
        FieldElement(val % FIELD_MODULUS)
    }

    pub fn value(&self) -> u64 {
        self.0
    }

    fn reduce(val: u128) -> u64 {
        let p = FIELD_MODULUS as u128;
        (val % p) as u64
    }

    pub fn inverse(&self) -> Option<Self> {
        if self.0 == 0 {
            return None;
        }
        Some(self.pow(FIELD_MODULUS - 2))
    }

    pub fn pow(&self, mut exp: u64) -> Self {
        let mut result = FieldElement::ONE;
        let mut base = *self;

        while exp > 0 {
            if exp & 1 == 1 {
                result = result * base;
            }
            base = base * base;
            exp >>= 1;
        }
        result
    }
}

impl Add for FieldElement {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        let sum = (self.0 as u128) + (rhs.0 as u128);
        FieldElement(Self::reduce(sum))
    }
}

impl Sub for FieldElement {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        if self.0 >= rhs.0 {
            FieldElement(self.0 - rhs.0)
        } else {
            FieldElement(FIELD_MODULUS - (rhs.0 - self.0))
        }
    }
}

impl Mul for FieldElement {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        let prod = (self.0 as u128) * (rhs.0 as u128);
        FieldElement(Self::reduce(prod))
    }
}

impl From<u64> for FieldElement {
    fn from(val: u64) -> Self {
        FieldElement::new(val)
    }
}

// ============================================================================
// POLYNOMIAL OPERATIONS
// ============================================================================

#[derive(Clone, Debug)]
pub struct Polynomial {
    pub coeffs: Vec<FieldElement>,
}

impl Polynomial {
    pub fn new(coeffs: Vec<FieldElement>) -> Self {
        Self { coeffs }
    }

    pub fn degree(&self) -> usize {
        self.coeffs.len().saturating_sub(1)
    }

    pub fn eval(&self, x: FieldElement) -> FieldElement {
        let mut result = FieldElement::ZERO;
        for &coeff in self.coeffs.iter().rev() {
            result = result * x + coeff;
        }
        result
    }

    pub fn interpolate(points: &[(FieldElement, FieldElement)]) -> Self {
        let n = points.len();
        let mut result = vec![FieldElement::ZERO; n];

        for i in 0..n {
            let (xi, yi) = points[i];
            let mut basis = vec![yi];

            for j in 0..n {
                if i != j {
                    let (xj, _) = points[j];
                    let denom = (xi - xj).inverse().unwrap();

                    let mut new_basis = vec![FieldElement::ZERO; basis.len() + 1];
                    for (k, &coeff) in basis.iter().enumerate() {
                        new_basis[k + 1] = new_basis[k + 1] + coeff;
                        new_basis[k] = new_basis[k] - coeff * xj;
                    }
                    for coeff in &mut new_basis {
                        *coeff = *coeff * denom;
                    }
                    basis = new_basis;
                }
            }
            for (k, coeff) in basis.iter().enumerate() {
                if k < result.len() {
                    result[k] = result[k] + *coeff;
                }
            }
        }
        Polynomial::new(result)
    }
}

/// Znajdź generator F_p* (p-1 = 2^27·3·5).
fn primitive_root() -> FieldElement {
    let phi = FIELD_MODULUS - 1;
    let factors: [u64; 3] = [2, 3, 5];
    let candidates: [u64; 12] = [3, 5, 7, 10, 11, 13, 17, 19, 23, 29, 31, 37];
    'outer: for &g in &candidates {
        let cand = FieldElement::new(g);
        for &q in &factors {
            if cand.pow(phi / q) == FieldElement::ONE {
                continue 'outer;
            }
        }
        return cand;
    }
    panic!("no primitive root found");
}

// ============================================================================
// MERKLE TREE (SHA-3 based)
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MerkleTree {
    pub root: [u8; 32],
    layers: Vec<Vec<[u8; 32]>>,
}

impl MerkleTree {
    pub fn build(leaves: &[[u8; 32]]) -> Self {
        if leaves.is_empty() {
            return Self {
                root: [0u8; 32],
                layers: vec![],
            };
        }

        let mut layers = vec![leaves.to_vec()];
        while layers.last().unwrap().len() > 1 {
            let current = layers.last().unwrap();
            let mut next = Vec::new();
            for chunk in current.chunks(2) {
                let left = chunk[0];
                let right = if chunk.len() > 1 { chunk[1] } else { chunk[0] };
                let mut h = Sha3_256::new();
                h.update(b"MERKLE");
                h.update(&left);
                h.update(&right);
                let parent: [u8; 32] = h.finalize().into();
                next.push(parent);
            }
            layers.push(next);
        }
        let root = layers.last().unwrap()[0];
        Self { root, layers }
    }

    pub fn prove(&self, index: usize) -> MerkleProof {
        let mut siblings = Vec::new();
        let mut idx = index;
        for layer in &self.layers[..self.layers.len() - 1] {
            let sibling_idx = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
            let sibling = layer.get(sibling_idx).copied().unwrap_or(layer[idx]);
            siblings.push(sibling);
            idx /= 2;
        }
        MerkleProof { leaf_index: index, siblings }
    }

    pub fn verify(root: &[u8; 32], proof: &MerkleProof, leaf: &[u8; 32]) -> bool {
        let mut current = *leaf;
        let mut idx = proof.leaf_index;

        for sibling in &proof.siblings {
            let (left, right) = if idx % 2 == 0 { (current, *sibling) } else { (*sibling, current) };
            let mut h = Sha3_256::new();
            h.update(b"MERKLE");
            h.update(&left);
            h.update(&right);
            current = h.finalize().into();
            idx /= 2;
        }
        &current == root
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MerkleProof {
    pub leaf_index: usize,
    pub siblings: Vec<[u8; 32]>,
}

// ============================================================================
// FRI (Fast Reed-Solomon IOP)
// ============================================================================

#[derive(Clone, Debug)]
pub struct FRIConfig {
    pub blowup_factor: usize,
    pub num_queries: usize,
    pub fold_factor: usize,
}

impl FRIConfig {
    pub fn default() -> Self {
        Self {
            blowup_factor: 8,
            num_queries: 40,
            fold_factor: 2,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FRIProof {
    pub layer_commitments: Vec<[u8; 32]>,
    pub final_poly: Vec<FieldElement>,
    pub queries: Vec<FRIQuery>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FRIQuery {
    pub index: usize,
    pub layer_values: Vec<FieldElement>,
    pub merkle_proofs: Vec<MerkleProof>,
}

pub struct FRIProver {
    config: FRIConfig,
}

impl FRIProver {
    pub fn new(config: FRIConfig) -> Self {
        Self { config }
    }

    pub fn commit(&self, poly: &Polynomial, domain_size: usize) -> (FRIProof, Vec<MerkleTree>) {
        let mut layer_vals: Vec<Vec<FieldElement>> = Vec::new();
        let evaluations = self.evaluate_on_domain(poly, domain_size);
        layer_vals.push(evaluations.clone());

        let leaves: Vec<[u8; 32]> = evaluations.iter().map(|&val| hash_field_element(val)).collect();
        let tree = MerkleTree::build(&leaves);
        let mut trees = vec![tree.clone()];
        let mut layer_commitments = vec![tree.root];

        let mut current_evaluations = evaluations;
        let num_layers = (domain_size.trailing_zeros() as usize) - 1;

        for _ in 0..num_layers {
            let folded = self.fold_layer(&current_evaluations);
            if folded.len() <= 4 {
                break;
            }
            let leaves: Vec<[u8; 32]> = folded.iter().map(|&val| hash_field_element(val)).collect();
            let tree = MerkleTree::build(&leaves);
            trees.push(tree.clone());
            layer_commitments.push(tree.root);
            layer_vals.push(folded.clone());
            current_evaluations = folded;
        }

        let final_poly = current_evaluations;
        let queries = self.generate_queries_with_merkle(&trees, &layer_vals);

        let proof = FRIProof {
            layer_commitments,
            final_poly,
            queries,
        };
        (proof, trees)
    }

    fn evaluate_on_domain(&self, poly: &Polynomial, size: usize) -> Vec<FieldElement> {
        let mut evaluations = Vec::with_capacity(size);
        let generator = self.get_generator(size);
        let mut x = FieldElement::ONE;
        for _ in 0..size {
            evaluations.push(poly.eval(x));
            x = x * generator;
        }
        evaluations
    }

    fn get_generator(&self, size: usize) -> FieldElement {
        assert!(size.is_power_of_two(), "Domain size must be power of 2");
        let log_size = size.trailing_zeros() as usize;
        assert!(log_size <= MAX_2_ADIC_ORDER, "Domain size 2^{} exceeds max 2^{}", log_size, MAX_2_ADIC_ORDER);

        if size == 1 {
            return FieldElement::ONE;
        }

        let exponent = (FIELD_MODULUS - 1) / (size as u64);
        let omega = primitive_root().pow(exponent);

        let omega_to_size = omega.pow(size as u64);
        assert!(omega_to_size == FieldElement::ONE, "ω^size != 1");
        if size > 1 {
            let omega_to_half = omega.pow((size / 2) as u64);
            assert!(omega_to_half != FieldElement::ONE, "ord(ω) < size");
        }
        omega
    }

    fn fold_layer(&self, evals: &[FieldElement]) -> Vec<FieldElement> {
        let n = evals.len();
        assert!(n.is_power_of_two() && n >= 2, "fold expects N=2^k, N>=2");
        let half = n / 2;
        let two_inv = FieldElement::new(2).inverse().unwrap();
        let mut out = Vec::with_capacity(half);
        for i in 0..half {
            out.push((evals[i] + evals[i + half]) * two_inv);
        }
        out
    }

    fn generate_queries_with_merkle(
        &self,
        trees: &[MerkleTree],
        layers: &[Vec<FieldElement>],
    ) -> Vec<FRIQuery> {
        let mut queries = Vec::new();

        let mut hasher = Sha3_256::new();
        hasher.update(b"FRI_QUERIES");
        for tree in trees {
            hasher.update(&tree.root);
        }
        let seed: [u8; 32] = hasher.finalize().into();
        let domain_size = layers.first().map(|v| v.len()).unwrap_or(0);

        for i in 0..self.config.num_queries {
            let mut h = Sha3_256::new();
            h.update(&seed);
            h.update(&(i as u64).to_le_bytes());
            let hash_bytes: [u8; 32] = h.finalize().into();
            let index = u64::from_le_bytes(hash_bytes[0..8].try_into().unwrap()) as usize % domain_size;

            let mut vals = Vec::with_capacity(trees.len());
            let mut proofs = Vec::with_capacity(trees.len());
            let mut idx = index;
            for (lvl, tree) in trees.iter().enumerate() {
                let cur_len = layers[lvl].len();
                let idx_lvl = idx % cur_len;
                vals.push(layers[lvl][idx_lvl]);
                proofs.push(tree.prove(idx_lvl));
                if cur_len > 1 {
                    idx = idx_lvl % (cur_len / 2);
                }
            }
            queries.push(FRIQuery { index, layer_values: vals, merkle_proofs: proofs });
        }
        queries
    }
}

pub struct FRIVerifier {
    config: FRIConfig,
}

impl FRIVerifier {
    pub fn new(config: FRIConfig) -> Self {
        Self { config }
    }

    pub fn verify(&self, proof: &FRIProof, _claimed_degree: usize) -> bool {
        if proof.layer_commitments.is_empty() {
            return false;
        }
        if proof.final_poly.len() > 4 {
            return false;
        }
        for query in &proof.queries {
            if !self.verify_query(query, proof) {
                return false;
            }
        }
        true
    }

    fn verify_query(&self, query: &FRIQuery, proof: &FRIProof) -> bool {
        if proof.layer_commitments.len() != query.merkle_proofs.len() {
            return false;
        }
        for (lvl, root) in proof.layer_commitments.iter().enumerate() {
            let leaf = hash_field_element(query.layer_values[lvl]);
            if !MerkleTree::verify(root, &query.merkle_proofs[lvl], &leaf) {
                return false;
            }
        }
        // TODO: algebraiczna weryfikacja foldingu (wymaga par wartości).
        true
    }
}

// ============================================================================
// STARK PROOF SYSTEM
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct STARKProof {
    pub trace_commitment: [u8; 32],
    pub constraint_commitment: [u8; 32],
    pub fri_proof: FRIProof,
    pub public_inputs: Vec<u64>,
}

impl STARKProof {
    pub fn size_bytes(&self) -> usize {
        let mut size = 32;
        size += 32;
        size += self.fri_proof.layer_commitments.len() * 32;
        size += self.fri_proof.final_poly.len() * 8;
        size += self.fri_proof.queries.len() * 100;
        size += self.public_inputs.len() * 8;
        size
    }
}

pub struct STARKProver {
    fri_prover: FRIProver,
}

// ============================================================================
// PUBLIC INPUTS ENCODING
// ============================================================================

pub const RANGE_PROOF_PUBLIC_INPUTS_LEN: usize = 5;

pub fn encode_range_public_inputs(value: u64, commitment: &Hash32) -> Vec<u64> {
    let mut out = Vec::with_capacity(RANGE_PROOF_PUBLIC_INPUTS_LEN);
    out.push(value);
    for i in 0..4 {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&commitment[8 * i..8 * (i + 1)]);
        out.push(u64::from_le_bytes(buf));
    }
    out
}

pub fn decode_commitment_from_public_inputs(inputs: &[u64]) -> Option<Hash32> {
    if inputs.len() < RANGE_PROOF_PUBLIC_INPUTS_LEN {
        return None;
    }
    let mut c = [0u8; 32];
    for i in 0..4 {
        c[8 * i..8 * (i + 1)].copy_from_slice(&inputs[1 + i].to_le_bytes());
    }
    Some(c)
}

// ============================================================================
// STARK PROVER
// ============================================================================

impl STARKProver {
    pub fn new() -> Self {
        let config = FRIConfig::default();
        Self { fri_prover: FRIProver::new(config) }
    }

    #[deprecated(note = "Use prove_range_with_commitment(value, commitment) instead")]
    pub fn prove_range(value: u64) -> STARKProof {
        let prover = Self::new();

        let trace = prover.generate_range_trace(value);
        let trace_leaves: Vec<[u8; 32]> = trace.iter().map(|&val| hash_field_element(val)).collect();
        let trace_tree = MerkleTree::build(&trace_leaves);

        let constraint_poly = prover.build_constraint_poly(&trace);
        let constraint_evals = prover.fri_prover.evaluate_on_domain(&constraint_poly, 128);
        let constraint_leaves: Vec<[u8; 32]> =
            constraint_evals.iter().map(|&v| hash_field_element(v)).collect();
        let constraint_tree = MerkleTree::build(&constraint_leaves);

        let (fri_proof, _trees) = prover.fri_prover.commit(&constraint_poly, 128);

        STARKProof {
            trace_commitment: trace_tree.root,
            constraint_commitment: constraint_tree.root,
            fri_proof,
            public_inputs: vec![value],
        }
    }

    pub fn prove_range_with_commitment(value: u64, commitment: &Hash32) -> STARKProof {
        let prover = Self::new();

        let trace = prover.generate_range_trace(value);
        let trace_leaves: Vec<[u8; 32]> = trace.iter().map(|&val| hash_field_element(val)).collect();
        let trace_tree = MerkleTree::build(&trace_leaves);

        let constraint_poly = prover.build_constraint_poly(&trace);
        let constraint_evals = prover.fri_prover.evaluate_on_domain(&constraint_poly, 128);
        let constraint_leaves: Vec<[u8; 32]> =
            constraint_evals.iter().map(|&v| hash_field_element(v)).collect();
        let constraint_tree = MerkleTree::build(&constraint_leaves);

        let (fri_proof, _trees) = prover.fri_prover.commit(&constraint_poly, 128);

        let public_inputs = encode_range_public_inputs(value, commitment);

        STARKProof {
            trace_commitment: trace_tree.root,
            constraint_commitment: constraint_tree.root,
            fri_proof,
            public_inputs,
        }
    }

    fn generate_range_trace(&self, value: u64) -> Vec<FieldElement> {
        let mut trace = Vec::with_capacity(64);
        for i in 0..64 {
            let bit = (value >> i) & 1;
            trace.push(FieldElement::new(bit));
        }
        trace
    }

    fn build_constraint_poly(&self, trace: &[FieldElement]) -> Polynomial {
        let mut points = Vec::new();
        for (i, &val) in trace.iter().enumerate() {
            let x = FieldElement::new(i as u64);
            let constraint = val * (val - FieldElement::ONE);
            points.push((x, constraint));
        }
        Polynomial::interpolate(&points)
    }
}

pub struct STARKVerifier {
    fri_verifier: FRIVerifier,
}

impl STARKVerifier {
    pub fn new() -> Self {
        let config = FRIConfig::default();
        Self { fri_verifier: FRIVerifier::new(config) }
    }

    pub fn verify(proof: &STARKProof) -> bool {
        let verifier = Self::new();

        if proof.trace_commitment == [0u8; 32] {
            return false;
        }
        if proof.constraint_commitment == [0u8; 32] {
            return false;
        }
        if !verifier.fri_verifier.verify(&proof.fri_proof, 64) {
            return false;
        }
        if proof.public_inputs.is_empty() {
            return false;
        }
        true
    }
}

// ============================================================================
// UTILITIES
// ============================================================================

fn hash_field_element(elem: FieldElement) -> [u8; 32] {
    let mut h = Sha3_256::new();
    h.update(b"FIELD");
    h.update(&elem.value().to_le_bytes());
    h.finalize().into()
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_arithmetic() {
        let a = FieldElement::new(100);
        let b = FieldElement::new(50);
        let sum = a + b;
        assert_eq!(sum.value(), 150);
        let diff = a - b;
        assert_eq!(diff.value(), 50);
        let prod = a * b;
        assert_eq!(prod.value(), 5000);
    }

    #[test]
    fn test_field_inverse() {
        let a = FieldElement::new(7);
        let a_inv = a.inverse().unwrap();
        let prod = a * a_inv;
        assert_eq!(prod, FieldElement::ONE);
    }

    #[test]
    fn test_polynomial_eval() {
        let poly = Polynomial::new(vec![FieldElement::new(1), FieldElement::new(2), FieldElement::new(3)]);
        let result = poly.eval(FieldElement::new(2));
        assert_eq!(result.value(), 17);
    }

    #[test]
    fn test_merkle_tree() {
        let leaves = vec![[1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32]];
        let tree = MerkleTree::build(&leaves);
        let proof = tree.prove(2);
        assert!(MerkleTree::verify(&tree.root, &proof, &leaves[2]));
    }

    #[test]
    fn test_fri_basic() {
        let config = FRIConfig::default();
        let prover = FRIProver::new(config);
        let poly = Polynomial::new(vec![FieldElement::ZERO, FieldElement::ONE]);
        let (proof, _trees) = prover.commit(&poly, 16);
        assert!(!proof.layer_commitments.is_empty());
        assert!(!proof.final_poly.is_empty());
        assert!(!proof.queries.is_empty());
    }

    #[test]
    fn test_stark_range_proof() {
        let value = 12345u64;
        let proof = STARKProver::prove_range(value);
        assert_eq!(proof.public_inputs[0], value);
        assert!(!proof.fri_proof.layer_commitments.is_empty());
        let valid = STARKVerifier::verify(&proof);
        assert!(valid);
    }

    #[test]
    fn test_stark_proof_size() {
        let proof = STARKProver::prove_range(99999);
        let size = proof.size_bytes();
        assert!(size < 500_000); // Realistic: ~100-400 KB
    }

    #[test]
    fn test_stark_performance() {
        use std::time::Instant;
        let value = 777777u64;
        let start = Instant::now();
        let proof = STARKProver::prove_range(value);
        let prove_time = start.elapsed();
        let start = Instant::now();
        let valid = STARKVerifier::verify(&proof);
        let verify_time = start.elapsed();
        assert!(valid);
        // Realistic expectations (not optimized):
        assert!(prove_time.as_millis() < 2000);  // < 2s prove
        assert!(verify_time.as_millis() < 500);  // < 500ms verify
    }
}
