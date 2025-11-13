//! CPU-only Proof System
//! 
//! Micro PoW + Proof generation tracking for trust building
//! NO GPU - CPU verification only!

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use crate::core::Hash32;
use tiny_keccak::{Hasher, Shake};

/// Micro PoW configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MicroPowParams {
    /// Target difficulty (leading zeros in bits)
    pub difficulty_bits: u8,
    
    /// Max iterations before giving up
    pub max_iterations: u64,
}

impl Default for MicroPowParams {
    fn default() -> Self {
        Self {
            difficulty_bits: 16, // ~65k hashes avg
            max_iterations: 1_000_000,
        }
    }
}

/// Proof-of-Work result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PowProof {
    pub nonce: u64,
    pub hash: Hash32,
    pub iterations: u64,
}

/// Proof generation metrics (for trust building)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProofMetrics {
    /// Number of Bulletproofs generated
    pub bp_generated: u32,
    
    /// Number of ZK proofs generated
    pub zk_generated: u32,
    
    /// CPU time spent on proof generation (ms)
    pub cpu_time_ms: u64,
    
    /// Micro PoW iterations performed
    pub pow_iterations: u64,
}

/// CPU-only micro PoW
/// 
/// Uses SHAKE256 (CPU-friendly, no GPU advantage)
pub fn mine_micro_pow(
    data: &[u8],
    params: &MicroPowParams,
) -> Option<PowProof> {
    let target_zeros = params.difficulty_bits;
    let target_mask = create_difficulty_mask(target_zeros);
    
    for nonce in 0..params.max_iterations {
        let hash = compute_pow_hash(data, nonce);
        
        // Check if hash meets difficulty target
        if check_difficulty(&hash, &target_mask) {
            return Some(PowProof {
                nonce,
                hash,
                iterations: nonce + 1,
            });
        }
    }
    
    None
}

/// Verify micro PoW
pub fn verify_micro_pow(
    data: &[u8],
    proof: &PowProof,
    params: &MicroPowParams,
) -> bool {
    // Recompute hash
    let hash = compute_pow_hash(data, proof.nonce);
    
    // Check hash matches
    if hash != proof.hash {
        return false;
    }
    
    // Check difficulty
    let target_mask = create_difficulty_mask(params.difficulty_bits);
    check_difficulty(&hash, &target_mask)
}

/// Compute PoW hash (CPU-only: SHAKE256)
fn compute_pow_hash(data: &[u8], nonce: u64) -> Hash32 {
    let mut sh = Shake::v256();
    sh.update(b"MICRO_POW");
    sh.update(data);
    sh.update(&nonce.to_le_bytes());
    let mut out = [0u8; 32];
    sh.finalize(&mut out);
    out
}

/// Create difficulty mask for target
fn create_difficulty_mask(bits: u8) -> Hash32 {
    let mut mask = [0xFFu8; 32];
    let full_bytes = (bits / 8) as usize;
    let remaining_bits = bits % 8;
    
    // Zero out full bytes
    for i in 0..full_bytes.min(32) {
        mask[i] = 0;
    }
    
    // Handle remaining bits
    if full_bytes < 32 && remaining_bits > 0 {
        let shift = 8 - remaining_bits;
        mask[full_bytes] = (0xFF >> remaining_bits) << shift;
    }
    
    mask
}

/// Check if hash meets difficulty
fn check_difficulty(hash: &Hash32, mask: &Hash32) -> bool {
    for i in 0..32 {
        if (hash[i] & !mask[i]) != 0 {
            return false;
        }
    }
    true
}

/// Calculate trust reward for proof generation
/// 
/// Formula: trust_delta = base_reward × (bp_weight × bp_count + zk_weight × zk_count + pow_weight × pow_quality)
pub fn calculate_proof_trust_reward(
    metrics: &ProofMetrics,
    bp_weight: f64,
    zk_weight: f64,
    pow_weight: f64,
    base_reward: f64,
) -> f64 {
    let bp_score = bp_weight * (metrics.bp_generated as f64);
    let zk_score = zk_weight * (metrics.zk_generated as f64);
    
    // PoW quality: more iterations = more work
    let pow_quality = (metrics.pow_iterations as f64).sqrt() / 1000.0;
    let pow_score = pow_weight * pow_quality;
    
    base_reward * (bp_score + zk_score + pow_score).min(10.0) // Cap at 10x
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_micro_pow_easy() {
        let params = MicroPowParams {
            difficulty_bits: 8,
            max_iterations: 10_000,
        };
        
        let data = b"test_block";
        let proof = mine_micro_pow(data, &params).expect("Should find proof");
        
        assert!(verify_micro_pow(data, &proof, &params));
        println!("Found proof in {} iterations", proof.iterations);
    }
    
    #[test]
    fn test_difficulty_mask() {
        let mask = create_difficulty_mask(16);
        assert_eq!(mask[0], 0);
        assert_eq!(mask[1], 0);
        assert_eq!(mask[2], 0xFF);
    }
    
    #[test]
    fn test_proof_trust_reward() {
        let metrics = ProofMetrics {
            bp_generated: 10,
            zk_generated: 5,
            cpu_time_ms: 5000,
            pow_iterations: 65536,
        };
        
        let reward = calculate_proof_trust_reward(
            &metrics,
            0.1,  // bp_weight
            0.2,  // zk_weight
            0.05, // pow_weight
            1.0,  // base_reward
        );
        
        assert!(reward > 0.0);
        println!("Trust reward: {:.4}", reward);
    }
}
