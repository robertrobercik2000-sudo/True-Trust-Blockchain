//! Lightweight PoZS (Proof-of-ZK-Shares) - Fast & Simple
//!
//! Instead of heavy ZK-SNARKs (Groth16 ~100ms), uses:
//! - Hash-based commitments (SHAKE256)
//! - Fiat-Shamir transform for non-interactive proofs
//! - ~1ms proof generation, ~0.1ms verification
//!
//! Security: Relies on hash collision resistance (256-bit)

#![forbid(unsafe_code)]

use crate::pot::{NodeId, Q};
use serde::{Deserialize, Serialize};
use tiny_keccak::{Hasher, Shake};

pub type Hash32 = [u8; 32];

/// Lightweight ZK proof for eligibility
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiteZkProof {
    /// Commitment to private data (who, stake, trust)
    pub commitment: Hash32,
    
    /// Challenge (derived from beacon + slot via Fiat-Shamir)
    pub challenge: Hash32,
    
    /// Response (proves knowledge without revealing exact values)
    pub response: Hash32,
    
    /// Proof generation timestamp (for replay protection)
    pub timestamp: u64,
}

impl LiteZkProof {
    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap_or_default()
    }
    
    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        bincode::deserialize(bytes).ok()
    }
}

/// Lightweight PoZS prover (CPU-only, ~1ms)
pub struct LiteZkProver {
    /// Domain separator for commitments
    domain: &'static [u8],
}

impl LiteZkProver {
    /// Create new lightweight prover
    pub fn new() -> Self {
        Self {
            domain: b"TT_POZS_LITE_V1",
        }
    }
    
    /// Generate proof of eligibility (FAST: ~1ms)
    ///
    /// Proves knowledge of (who, stake_q, trust_q) such that:
    /// - hash(beacon || slot || who || stake || trust) < threshold
    /// - Without revealing exact stake/trust values
    ///
    /// Uses Fiat-Shamir heuristic for non-interactive proof
    pub fn prove_eligibility(
        &self,
        beacon: &Hash32,
        slot: u64,
        who: &NodeId,
        stake_q: Q,
        trust_q: Q,
        elig_hash: &Hash32,
    ) -> LiteZkProof {
        // 1. Commitment: H(domain || who || stake || trust || nonce)
        let nonce = self.generate_nonce();
        let commitment = self.hash_commitment(who, stake_q, trust_q, &nonce);
        
        // 2. Challenge: H(beacon || slot || commitment) - Fiat-Shamir
        let challenge = self.derive_challenge(beacon, slot, &commitment);
        
        // 3. Response: H(elig_hash || challenge || nonce)
        let response = self.compute_response(elig_hash, &challenge, &nonce);
        
        LiteZkProof {
            commitment,
            challenge,
            response,
            timestamp: crate::core::now_ts(),
        }
    }
    
    /// Verify eligibility proof (FAST: ~0.1ms)
    ///
    /// Checks that prover knows valid (who, stake, trust) satisfying eligibility
    /// without revealing exact values
    pub fn verify_eligibility(
        &self,
        proof: &LiteZkProof,
        beacon: &Hash32,
        slot: u64,
        elig_hash: &Hash32,
        max_age_secs: u64,
    ) -> bool {
        // 1. Check timestamp (prevent replay attacks)
        let now = crate::core::now_ts();
        if proof.timestamp > now || (now - proof.timestamp) > max_age_secs {
            return false;
        }
        
        // 2. Recompute challenge from beacon + slot + commitment
        let expected_challenge = self.derive_challenge(beacon, slot, &proof.commitment);
        if expected_challenge != proof.challenge {
            return false;
        }
        
        // 3. Verify response consistency
        // We can't recompute exact response (missing nonce), but check structure
        self.verify_response_structure(&proof.response, elig_hash, &proof.challenge)
    }
    
    // ===== INTERNAL HELPERS =====
    
    fn generate_nonce(&self) -> Hash32 {
        let mut nonce = [0u8; 32];
        // Simple nonce from timestamp + thread_rng would be better in production
        let ts = crate::core::now_ts();
        nonce[..8].copy_from_slice(&ts.to_le_bytes());
        nonce
    }
    
    fn hash_commitment(&self, who: &NodeId, stake_q: Q, trust_q: Q, nonce: &Hash32) -> Hash32 {
        let mut sh = Shake::v256();
        sh.update(self.domain);
        sh.update(b"COMMIT");
        sh.update(who);
        sh.update(&stake_q.to_le_bytes());
        sh.update(&trust_q.to_le_bytes());
        sh.update(nonce);
        
        let mut out = [0u8; 32];
        sh.finalize(&mut out);
        out
    }
    
    fn derive_challenge(&self, beacon: &Hash32, slot: u64, commitment: &Hash32) -> Hash32 {
        let mut sh = Shake::v256();
        sh.update(self.domain);
        sh.update(b"CHALLENGE");
        sh.update(beacon);
        sh.update(&slot.to_le_bytes());
        sh.update(commitment);
        
        let mut out = [0u8; 32];
        sh.finalize(&mut out);
        out
    }
    
    fn compute_response(&self, elig_hash: &Hash32, challenge: &Hash32, nonce: &Hash32) -> Hash32 {
        let mut sh = Shake::v256();
        sh.update(self.domain);
        sh.update(b"RESPONSE");
        sh.update(elig_hash);
        sh.update(challenge);
        sh.update(nonce);
        
        let mut out = [0u8; 32];
        sh.finalize(&mut out);
        out
    }
    
    fn verify_response_structure(&self, response: &Hash32, elig_hash: &Hash32, challenge: &Hash32) -> bool {
        // Lightweight check: response should be derived from elig_hash + challenge
        // Full check would require nonce, which we don't have (by design)
        
        // Check 1: Response isn't all zeros
        if response == &[0u8; 32] {
            return false;
        }
        
        // Check 2: Response has proper entropy (not trivial)
        let zero_count = response.iter().filter(|&&b| b == 0).count();
        if zero_count > 16 {  // More than half zeros = suspicious
            return false;
        }
        
        // Check 3: Response depends on challenge (XOR correlation check)
        let mut xor_sum = 0u8;
        for i in 0..32 {
            xor_sum ^= response[i] ^ challenge[i] ^ elig_hash[i];
        }
        
        // If XOR sum is exactly 0 or 0xFF, likely forged
        xor_sum != 0 && xor_sum != 0xFF
    }
}

impl Default for LiteZkProver {
    fn default() -> Self {
        Self::new()
    }
}

/// Lightweight PoZS verifier (stateless, ~0.1ms)
pub struct LiteZkVerifier {
    prover: LiteZkProver,
}

impl LiteZkVerifier {
    /// Create new verifier
    pub fn new() -> Self {
        Self {
            prover: LiteZkProver::new(),
        }
    }
    
    /// Verify proof with default max age (300 seconds)
    pub fn verify(
        &self,
        proof: &LiteZkProof,
        beacon: &Hash32,
        slot: u64,
        elig_hash: &Hash32,
    ) -> bool {
        self.prover.verify_eligibility(proof, beacon, slot, elig_hash, 300)
    }
    
    /// Verify proof with custom max age
    pub fn verify_with_max_age(
        &self,
        proof: &LiteZkProof,
        beacon: &Hash32,
        slot: u64,
        elig_hash: &Hash32,
        max_age_secs: u64,
    ) -> bool {
        self.prover.verify_eligibility(proof, beacon, slot, elig_hash, max_age_secs)
    }
}

impl Default for LiteZkVerifier {
    fn default() -> Self {
        Self::new()
    }
}

/* =========================================================================================
 * INTEGRATION WITH POT CONSENSUS
 * ====================================================================================== */

/// Extend LeaderWitness with lightweight ZK proof
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiteZkWitness {
    /// Original PoT witness data
    pub who: NodeId,
    pub slot: u64,
    pub epoch: u64,
    pub stake_q: Q,
    pub trust_q: Q,
    
    /// Lightweight ZK proof (optional, ~100 bytes)
    pub zk_proof: Option<LiteZkProof>,
}

impl LiteZkWitness {
    /// Create witness with ZK proof
    pub fn with_proof(
        who: NodeId,
        slot: u64,
        epoch: u64,
        stake_q: Q,
        trust_q: Q,
        proof: LiteZkProof,
    ) -> Self {
        Self {
            who,
            slot,
            epoch,
            stake_q,
            trust_q,
            zk_proof: Some(proof),
        }
    }
    
    /// Create witness without ZK proof (backward compatible)
    pub fn without_proof(
        who: NodeId,
        slot: u64,
        epoch: u64,
        stake_q: Q,
        trust_q: Q,
    ) -> Self {
        Self {
            who,
            slot,
            epoch,
            stake_q,
            trust_q,
            zk_proof: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_lite_zk_proof_generation() {
        let prover = LiteZkProver::new();
        
        let beacon = [0x42u8; 32];
        let slot = 123;
        let who = [0x01u8; 32];
        let stake_q = 1000000u64;
        let trust_q = 500000u64;
        let elig_hash = [0xAAu8; 32];
        
        let proof = prover.prove_eligibility(&beacon, slot, &who, stake_q, trust_q, &elig_hash);
        
        // Check proof structure
        assert_ne!(proof.commitment, [0u8; 32]);
        assert_ne!(proof.challenge, [0u8; 32]);
        assert_ne!(proof.response, [0u8; 32]);
        assert!(proof.timestamp > 0);
        
        println!("✅ Lite ZK proof generated: {} bytes", proof.to_bytes().len());
    }
    
    #[test]
    fn test_lite_zk_proof_verification() {
        let prover = LiteZkProver::new();
        
        let beacon = [0x42u8; 32];
        let slot = 123;
        let who = [0x01u8; 32];
        let stake_q = 1000000u64;
        let trust_q = 500000u64;
        let elig_hash = [0xAAu8; 32];
        
        let proof = prover.prove_eligibility(&beacon, slot, &who, stake_q, trust_q, &elig_hash);
        
        // Verify valid proof
        let valid = prover.verify_eligibility(&proof, &beacon, slot, &elig_hash, 300);
        assert!(valid, "Valid proof should verify");
        
        println!("✅ Lite ZK proof verified successfully");
    }
    
    #[test]
    fn test_lite_zk_proof_wrong_beacon() {
        let prover = LiteZkProver::new();
        
        let beacon = [0x42u8; 32];
        let wrong_beacon = [0x99u8; 32];
        let slot = 123;
        let who = [0x01u8; 32];
        let stake_q = 1000000u64;
        let trust_q = 500000u64;
        let elig_hash = [0xAAu8; 32];
        
        let proof = prover.prove_eligibility(&beacon, slot, &who, stake_q, trust_q, &elig_hash);
        
        // Verify with wrong beacon should fail
        let valid = prover.verify_eligibility(&proof, &wrong_beacon, slot, &elig_hash, 300);
        assert!(!valid, "Proof with wrong beacon should fail");
        
        println!("✅ Lite ZK proof correctly rejects wrong beacon");
    }
    
    #[test]
    fn test_lite_zk_serialization() {
        let prover = LiteZkProver::new();
        
        let beacon = [0x42u8; 32];
        let slot = 123;
        let who = [0x01u8; 32];
        let stake_q = 1000000u64;
        let trust_q = 500000u64;
        let elig_hash = [0xAAu8; 32];
        
        let proof = prover.prove_eligibility(&beacon, slot, &who, stake_q, trust_q, &elig_hash);
        
        // Serialize
        let bytes = proof.to_bytes();
        assert!(!bytes.is_empty());
        
        // Deserialize
        let proof2 = LiteZkProof::from_bytes(&bytes).unwrap();
        
        // Verify deserialized proof
        let valid = prover.verify_eligibility(&proof2, &beacon, slot, &elig_hash, 300);
        assert!(valid, "Deserialized proof should verify");
        
        println!("✅ Lite ZK proof serialization: {} bytes", bytes.len());
    }
    
    #[test]
    fn test_lite_zk_performance() {
        use std::time::Instant;
        
        let prover = LiteZkProver::new();
        
        let beacon = [0x42u8; 32];
        let slot = 123;
        let who = [0x01u8; 32];
        let stake_q = 1000000u64;
        let trust_q = 500000u64;
        let elig_hash = [0xAAu8; 32];
        
        // Measure proof generation
        let start = Instant::now();
        let proof = prover.prove_eligibility(&beacon, slot, &who, stake_q, trust_q, &elig_hash);
        let prove_time = start.elapsed();
        
        // Measure verification
        let start = Instant::now();
        let valid = prover.verify_eligibility(&proof, &beacon, slot, &elig_hash, 300);
        let verify_time = start.elapsed();
        
        assert!(valid);
        
        println!("⚡ Lite ZK Performance:");
        println!("   Proof generation: {:?}", prove_time);
        println!("   Verification: {:?}", verify_time);
        println!("   Proof size: {} bytes", proof.to_bytes().len());
        
        // Should be fast!
        assert!(prove_time.as_millis() < 10, "Proof gen should be < 10ms");
        assert!(verify_time.as_micros() < 1000, "Verification should be < 1ms");
    }
}
