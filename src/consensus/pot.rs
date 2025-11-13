//! PoT (Proof-of-Trust) consensus core logic

use super::types::*;
use super::snapshot::{EpochSnapshot, qmul, qdiv};
use crate::crypto::kmac256_hash;
use anyhow::{ensure, Result};

/// PoT consensus engine
pub struct PotConsensus {
    pub params: PotParams,
    pub trust_params: TrustParams,
}

impl PotConsensus {
    pub fn new(params: PotParams, trust_params: TrustParams) -> Self {
        Self { params, trust_params }
    }

    /// Compute eligibility hash for (beacon, slot, who)
    pub fn elig_hash(&self, beacon: &[u8; 32], slot: u64, who: &NodeId) -> u64 {
        let hash = kmac256_hash(b"ELIG.v1", &[beacon, &slot.to_le_bytes(), who]);
        let mut w = [0u8; 8];
        w.copy_from_slice(&hash[..8]);
        u64::from_be_bytes(w)
    }

    /// Compute threshold: (lambda × weight) / sum_weights
    pub fn compute_threshold_q(
        &self,
        weight_q: Q,
        sum_weights_q: u128,
    ) -> Option<Q> {
        let numerator = qmul(self.params.lambda_q, weight_q) as u128;
        let threshold = ((numerator << 32) / sum_weights_q) as Q;
        Some(threshold)
    }

    /// Verify leader eligibility
    pub fn verify_leader_eligibility(
        &self,
        snapshot: &EpochSnapshot,
        beacon: &[u8; 32],
        witness: &LeaderWitness,
    ) -> Result<bool> {
        // 1. Check weights root matches
        ensure!(
            witness.weights_root == snapshot.weights_root,
            "Weights root mismatch"
        );

        // 2. Get validator weight
        let weight = snapshot.get_weight(&witness.who)
            .ok_or_else(|| anyhow::anyhow!("Validator not in snapshot"))?;

        // 3. Compute threshold
        let threshold = self.compute_threshold_q(weight, snapshot.total_weight_q)
            .ok_or_else(|| anyhow::anyhow!("Threshold overflow"))?;

        // 4. Compute eligibility hash
        let elig = self.elig_hash(beacon, witness.slot, &witness.who);
        
        // 5. Check: elig < threshold × u64::MAX
        let bound = qmul(threshold, u64::MAX as Q);
        
        Ok(elig < bound)
    }

    /// Update trust after block validation
    pub fn update_trust(
        &self,
        trust_state: &mut TrustState,
        who: &NodeId,
        success: bool,
    ) {
        let current = trust_state.get_trust(who);
        
        let new_trust = if success {
            // Reward: increase trust
            let bonus = qmul(current, ONE_Q / 100); // +1%
            (current.saturating_add(bonus)).min(self.trust_params.max_trust_q)
        } else {
            // Penalty: decrease trust
            let penalty = qmul(current, ONE_Q / 10); // -10%
            (current.saturating_sub(penalty)).max(self.trust_params.min_trust_q)
        };

        trust_state.set_trust(who, new_trust);
    }
}

/// Verify leader eligibility (standalone function)
pub fn verify_leader_eligibility(
    params: &PotParams,
    snapshot: &EpochSnapshot,
    beacon: &[u8; 32],
    witness: &LeaderWitness,
) -> Result<bool> {
    let consensus = PotConsensus::new(params.clone(), TrustParams::default());
    consensus.verify_leader_eligibility(snapshot, beacon, witness)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_elig_hash() {
        let pot = PotConsensus::new(PotParams::default(), TrustParams::default());
        let beacon = [1u8; 32];
        let who = [2u8; 32];
        
        let h1 = pot.elig_hash(&beacon, 0, &who);
        let h2 = pot.elig_hash(&beacon, 0, &who);
        assert_eq!(h1, h2); // deterministic
    }
}
