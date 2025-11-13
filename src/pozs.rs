#![forbid(unsafe_code)]

//! PoZS (Proof-of-ZK-Shares) integration layer for PoT consensus
//!
//! This module provides ZK proof capabilities for leader eligibility verification.
//! It complements the existing PoT consensus by adding cryptographic proofs of:
//! - Leader eligibility (stake_q × trust_q ≥ threshold)
//! - Beacon randomness inclusion
//! - Merkle path validity (optional)
//!
//! The PoZS layer can operate in several modes:
//! 1. **Proof-of-Eligibility**: ZK proof that validator won the sortition
//! 2. **Proof-of-Shares**: Recursive aggregation of multiple eligibility proofs
//! 3. **Hybrid**: Optional ZK proofs that enhance but don't replace Merkle witnesses

use crate::pot::{NodeId, PotParams, Q, RandaoBeacon, Registry, TrustState, EpochSnapshot};
use thiserror::Error;

/* =========================================================================================
 * ZK PROOF TYPES
 * ====================================================================================== */

/// ZK proof scheme used for leader eligibility
#[derive(Clone, Debug)]
pub enum ZkScheme {
    /// Groth16 over BN254 (fast verification, ~200 bytes)
    Groth16BN254,
    /// PLONK over BLS12-381 (universal setup, ~400 bytes)
    PlonkBLS12,
    /// Nova/Halo2 recursive (aggregation-friendly)
    NovaRecursive,
}

/// Serialized ZK proof
#[derive(Clone, Debug)]
pub struct ZkProof {
    pub scheme: ZkScheme,
    pub proof_bytes: Vec<u8>,
}

/// Leader witness extended with optional ZK proof
#[derive(Clone, Debug)]
pub struct ZkLeaderWitness {
    pub who: NodeId,
    pub slot: u64,
    pub epoch: u64,
    /// Merkle root of epoch weights (stake_q × trust_q)
    pub weights_root: [u8; 32],
    /// Classical Merkle proof (backward compatible)
    pub merkle_proof: Option<Vec<u8>>,
    /// Validator's normalized stake
    pub stake_q: Q,
    /// Validator's trust score at snapshot
    pub trust_q: Q,
    /// Optional ZK proof of eligibility
    pub zk_proof: Option<ZkProof>,
}

/* =========================================================================================
 * PROOF GENERATION (stub interface)
 * ====================================================================================== */

/// ZK prover context (circuit + proving key)
pub struct ZkProver {
    scheme: ZkScheme,
    // In production: proving key, circuit parameters, etc.
    _proving_key: Vec<u8>,
}

impl ZkProver {
    /// Create a new ZK prover for the given scheme
    pub fn new(scheme: ZkScheme) -> Result<Self, ZkError> {
        // TODO: Load proving key from disk or trusted setup
        Ok(Self {
            scheme,
            _proving_key: Vec::new(),
        })
    }

    /// Generate ZK proof of leader eligibility
    ///
    /// Circuit proves:
    /// ```text
    /// public inputs: weights_root, beacon_value, threshold_q
    /// private inputs: who, stake_q, trust_q, merkle_path
    /// constraint: hash(beacon || slot || who) < bound(λ × stake_q × trust_q / Σweights)
    /// ```
    pub fn prove_eligibility(
        &self,
        _beacon_value: &[u8; 32],
        slot: u64,
        who: &NodeId,
        _stake_q: Q,
        _trust_q: Q,
        _threshold_q: Q,
    ) -> Result<ZkProof, ZkError> {
        // STUB: In production, this would:
        // 1. Construct circuit witness
        // 2. Run Groth16/PLONK prover
        // 3. Serialize proof to bytes
        
        eprintln!("[STUB] Generating ZK proof for slot={} validator={:02x?}...", 
                  slot, &who[..4]);
        
        Ok(ZkProof {
            scheme: self.scheme.clone(),
            proof_bytes: vec![0xDE, 0xAD, 0xBE, 0xEF], // Placeholder
        })
    }
}

/* =========================================================================================
 * PROOF VERIFICATION
 * ====================================================================================== */

/// ZK verifier context (circuit + verifying key)
pub struct ZkVerifier {
    _scheme: ZkScheme,
    // In production: verifying key, preprocessed circuit, etc.
    _verifying_key: Vec<u8>,
}

impl ZkVerifier {
    /// Create a new ZK verifier for the given scheme
    pub fn new(scheme: ZkScheme) -> Result<Self, ZkError> {
        // TODO: Load verifying key from embedded constant or config
        Ok(Self {
            _scheme: scheme,
            _verifying_key: Vec::new(),
        })
    }

    /// Verify ZK proof of leader eligibility
    ///
    /// Returns true if proof is valid for the given public inputs
    pub fn verify_eligibility(
        &self,
        proof: &ZkProof,
        _beacon_value: &[u8; 32],
        _weights_root: &[u8; 32],
        _threshold_q: Q,
    ) -> Result<bool, ZkError> {
        if !matches!(proof.scheme, ZkScheme::Groth16BN254) {
            return Err(ZkError::UnsupportedScheme);
        }

        // STUB: In production, this would:
        // 1. Deserialize proof from bytes
        // 2. Construct public inputs vector
        // 3. Run verifier pairing check
        // 4. Return verification result
        
        eprintln!("[STUB] Verifying ZK proof: {} bytes", proof.proof_bytes.len());
        
        Ok(true) // Placeholder
    }
}

/* =========================================================================================
 * HYBRID VERIFICATION: PoT + PoZS
 * ====================================================================================== */

/// Verify leader with optional ZK proof enhancement
///
/// This function performs **hybrid verification**:
/// 1. Classical PoT check (beacon + stake + trust)
/// 2. Optional ZK proof verification (if present)
///
/// Benefits of ZK enhancement:
/// - Lighter verification for full nodes (pairing check vs Merkle + hash)
/// - Recursive aggregation potential (Nova/Halo2)
/// - Privacy: hides exact stake_q/trust_q from block header
pub fn verify_leader_zk(
    reg: &Registry,
    epoch_snap: &EpochSnapshot,
    beacon: &RandaoBeacon,
    trust_state: &mut TrustState,
    params: &PotParams,
    witness: &ZkLeaderWitness,
    verifier: Option<&ZkVerifier>,
) -> Result<u128, ZkError> {
    // 1. Classical PoT verification (unchanged)
    let weight = verify_leader_classical(
        reg,
        epoch_snap,
        beacon,
        params,
        witness.epoch,
        witness.slot,
        &witness.who,
        witness.stake_q,
        witness.trust_q,
    )
    .ok_or(ZkError::NotEligible)?;

    // 2. Optional ZK proof verification
    if let (Some(proof), Some(ver)) = (&witness.zk_proof, verifier) {
        let beacon_value = beacon.value(witness.epoch, witness.slot);
        
        // Compute threshold (same as in classical path)
        let threshold_q = compute_threshold_q(
            params.lambda_q,
            witness.stake_q,
            witness.trust_q,
            epoch_snap.sum_weights_q,
        );

        let valid = ver.verify_eligibility(
            proof,
            &beacon_value,
            &witness.weights_root,
            threshold_q,
        )?;

        if !valid {
            return Err(ZkError::InvalidProof);
        }
    }

    // 3. Update trust (reward for valid block)
    trust_state.apply_block_reward(&witness.who, params.trust);

    Ok(weight)
}

/// Helper: compute eligibility threshold (same formula as pot.rs)
#[inline]
fn compute_threshold_q(lambda_q: Q, stake_q: Q, trust_q: Q, sum_weights_q: Q) -> Q {
    use crate::pot::ONE_Q;
    let sum = sum_weights_q.max(ONE_Q / 1_000_000);
    let wi = qmul(stake_q, qclamp01(trust_q));
    qclamp01(qmul(lambda_q, qdiv(wi, sum)))
}

/// Classical leader verification (duplicated from pot.rs for independence)
fn verify_leader_classical(
    reg: &Registry,
    epoch_snap: &EpochSnapshot,
    beacon: &RandaoBeacon,
    params: &PotParams,
    epoch: u64,
    slot: u64,
    who: &NodeId,
    stake_q: Q,
    trust_q: Q,
) -> Option<u128> {
    if !reg.is_active(who, params.min_bond) { return None; }
    if epoch != epoch_snap.epoch { return None; }
    if epoch_snap.sum_weights_q == 0 { return None; }

    let p_q = compute_threshold_q(params.lambda_q, stake_q, trust_q, epoch_snap.sum_weights_q);
    let b = beacon.value(epoch, slot);
    let y = elig_hash(&b, slot, who);
    if y > bound_u64(p_q) { return None; }

    let denom = u128::from(y).saturating_add(1);
    let weight = (u128::from(u64::MAX) + 1) / denom;
    Some(weight)
}

// Q32.32 helpers (duplicated for module independence)
#[inline]
fn qmul(a: Q, b: Q) -> Q {
    let z = (a as u128) * (b as u128);
    let shifted = z >> 32;
    shifted.min(u64::MAX as u128) as u64
}

#[inline]
fn qdiv(a: Q, b: Q) -> Q {
    if b == 0 { 0 } else {
        let z = (a as u128) << 32;
        (z / (b as u128)).min(u128::from(u64::MAX)) as u64
    }
}

#[inline]
fn qclamp01(x: Q) -> Q { 
    x.min(crate::pot::ONE_Q) 
}

#[inline]
fn elig_hash(beacon: &[u8; 32], slot: u64, who: &NodeId) -> u64 {
    use crate::crypto_kmac_consensus::kmac256_hash;
    let hash = kmac256_hash(b"ELIG.v1", &[
        beacon,
        &slot.to_le_bytes(),
        who,
    ]);
    let mut w = [0u8; 8];
    w.copy_from_slice(&hash[..8]);
    u64::from_be_bytes(w)
}

#[inline]
fn bound_u64(p_q: Q) -> u64 {
    (((p_q as u128) << 32).min(u128::from(u64::MAX))) as u64
}

/* =========================================================================================
 * RECURSIVE AGGREGATION (future work)
 * ====================================================================================== */

/// Aggregated proof for multiple blocks (Nova/Halo2)
#[derive(Clone, Debug)]
pub struct AggregatedProof {
    /// Number of blocks aggregated
    pub block_count: u64,
    /// Starting epoch/slot
    pub start_epoch: u64,
    pub start_slot: u64,
    /// Ending epoch/slot
    pub end_epoch: u64,
    pub end_slot: u64,
    /// Recursive proof
    pub proof: ZkProof,
}

impl AggregatedProof {
    /// Fold a new block proof into the aggregation
    pub fn fold(&mut self, _next_witness: &ZkLeaderWitness) -> Result<(), ZkError> {
        // TODO: Recursive folding with Nova/Halo2
        self.block_count += 1;
        Ok(())
    }

    /// Verify the aggregated proof
    pub fn verify(&self, _verifier: &ZkVerifier) -> Result<bool, ZkError> {
        // TODO: Verify recursive proof
        Ok(true)
    }
}

/* =========================================================================================
 * ERROR TYPES
 * ====================================================================================== */

#[derive(Debug, Error)]
pub enum ZkError {
    #[error("ZK proof scheme not supported")]
    UnsupportedScheme,
    #[error("ZK proof verification failed")]
    InvalidProof,
    #[error("validator not eligible for slot")]
    NotEligible,
    #[error("failed to generate proof: {0}")]
    ProofGenerationFailed(String),
}

/* =========================================================================================
 * INTEGRATION EXAMPLE
 * ====================================================================================== */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pot::q_from_basis_points;

    #[test]
    fn zk_witness_creation() {
        let who = [1u8; 32];
        let witness = ZkLeaderWitness {
            who,
            slot: 42,
            epoch: 1,
            weights_root: [0u8; 32],
            merkle_proof: None,
            stake_q: q_from_basis_points(5000), // 50%
            trust_q: q_from_basis_points(8000), // 80%
            zk_proof: None,
        };
        
        assert_eq!(witness.slot, 42);
        assert!(witness.zk_proof.is_none());
    }

    #[test]
    fn zk_prover_creation() {
        let prover = ZkProver::new(ZkScheme::Groth16BN254);
        assert!(prover.is_ok());
    }

    #[test]
    fn zk_verifier_creation() {
        let verifier = ZkVerifier::new(ZkScheme::Groth16BN254);
        assert!(verifier.is_ok());
    }

    #[test]
    fn stub_proof_generation() {
        let prover = ZkProver::new(ZkScheme::Groth16BN254).unwrap();
        let beacon = [7u8; 32];
        let who = [1u8; 32];
        
        let proof = prover.prove_eligibility(
            &beacon,
            42,
            &who,
            q_from_basis_points(5000),
            q_from_basis_points(8000),
            q_from_basis_points(1000),
        );
        
        assert!(proof.is_ok());
        assert_eq!(proof.unwrap().proof_bytes.len(), 4); // Stub proof
    }

    #[test]
    fn stub_proof_verification() {
        let verifier = ZkVerifier::new(ZkScheme::Groth16BN254).unwrap();
        let proof = ZkProof {
            scheme: ZkScheme::Groth16BN254,
            proof_bytes: vec![0xDE, 0xAD, 0xBE, 0xEF],
        };
        
        let result = verifier.verify_eligibility(
            &proof,
            &[7u8; 32],
            &[0u8; 32],
            q_from_basis_points(1000),
        );
        
        assert!(result.is_ok());
        assert!(result.unwrap()); // Stub returns true
    }
}

/* =========================================================================================
 * PRODUCTION NOTES
 * ====================================================================================== */

// To implement production PoZS:
//
// 1. **Circuit Design** (using arkworks or halo2):
//    - Public inputs: weights_root, beacon_value, threshold_q
//    - Private inputs: who, stake_q, trust_q, merkle_siblings
//    - Constraints:
//      a) Merkle path verification
//      b) Eligibility hash computation
//      c) Threshold comparison
//
// 2. **Proving Key Generation**:
//    - Run trusted setup (Groth16) or universal setup (PLONK)
//    - Embed verifying key in binary or load from config
//    - Cache proving key for validator nodes
//
// 3. **Integration Points**:
//    - pot_node.rs: Add `zk_prover: Option<ZkProver>` to PotNode
//    - SlotDecision: Include `zk_proof: Option<ZkProof>`
//    - Block header: Serialize ZkLeaderWitness
//
// 4. **Performance Targets**:
//    - Proof generation: <500ms on validator hardware
//    - Proof verification: <10ms on full node
//    - Proof size: <256 bytes (Groth16) or <400 bytes (PLONK)
//
// 5. **Recursive Aggregation** (advanced):
//    - Use Nova/Halo2 for folding multiple blocks
//    - Sync nodes verify 1 proof instead of N proofs
//    - Checkpoint proofs every 100 blocks
