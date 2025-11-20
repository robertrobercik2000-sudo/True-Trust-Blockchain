#![forbid(unsafe_code)]

//! Snapshot witness verification module
//! Provides compact witness format for weight verification

use crate::pot::{EpochSnapshot, MerkleProof, NodeId, Q, StakeQ};

/// Compact weight witness format (V1)
/// Contains minimal information needed to verify a node's weight in an epoch snapshot
#[derive(Clone, Debug)]
pub struct WeightWitnessV1 {
    pub who: NodeId,
    pub stake_q: StakeQ,
    pub trust_q: Q,
    pub leaf_index: u64,
    pub siblings: Vec<[u8; 32]>, // Merkle proof siblings
}

/// Extension trait for EpochSnapshot to verify compact witnesses
pub trait SnapshotWitnessExt {
    /// Verify a compact weight witness against this snapshot
    fn verify_witness(&self, wit: &WeightWitnessV1) -> bool;
}

impl SnapshotWitnessExt for EpochSnapshot {
    fn verify_witness(&self, wit: &WeightWitnessV1) -> bool {
        // Check that the witness matches the snapshot's data
        if wit.stake_q != self.stake_q_of(&wit.who) {
            return false;
        }
        if wit.trust_q != self.trust_q_of(&wit.who) {
            return false;
        }
        
        // Check leaf index matches
        match self.leaf_index_of(&wit.who) {
            Some(idx) if idx == wit.leaf_index => {},
            _ => return false,
        }
        
        // Verify Merkle proof
        let leaf = merkle_leaf_hash(&wit.who, wit.stake_q, wit.trust_q);
        let proof = MerkleProof {
            leaf_index: wit.leaf_index,
            siblings: wit.siblings.clone(),
        };
        verify_merkle(&proof, leaf, self.weights_root)
    }
}

#[inline]
fn merkle_leaf_hash(who: &NodeId, stake_q: StakeQ, trust_q: Q) -> [u8; 32] {
    // Use SHA256 for compatibility with pot.rs Merkle trees
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"WGT.v1");
    h.update(who);
    h.update(stake_q.to_le_bytes());
    h.update(trust_q.to_le_bytes());
    let out = h.finalize();
    let mut r = [0u8; 32];
    r.copy_from_slice(&out);
    r
}

#[inline]
fn merkle_parent(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    // Use SHA256 for compatibility with pot.rs Merkle trees
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"MRK.v1");
    h.update(a);
    h.update(b);
    let out = h.finalize();
    let mut r = [0u8; 32];
    r.copy_from_slice(&out);
    r
}

fn verify_merkle(proof: &MerkleProof, leaf: [u8; 32], root: [u8; 32]) -> bool {
    let mut acc = leaf;
    let mut idx = proof.leaf_index;
    for sib in &proof.siblings {
        acc = if (idx & 1) == 0 {
            merkle_parent(&acc, sib)
        } else {
            merkle_parent(sib, &acc)
        };
        idx >>= 1;
    }
    acc == root
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pot::{Registry, TrustParams, TrustState, q_from_basis_points, ONE_Q};

    fn nid(n: u8) -> NodeId {
        let mut id = [0u8; 32];
        id[0] = n;
        id
    }

    #[test]
    fn witness_verification() {
        let mut reg = Registry::default();
        let tp = TrustParams {
            alpha_q: q_from_basis_points(9900),
            beta_q: q_from_basis_points(100),
            init_q: q_from_basis_points(1000),
        };
        let mut ts = TrustState::default();
        let a = nid(1);
        reg.insert(a, 100, true);
        ts.set(a, q_from_basis_points(5000));
        
        let snap = EpochSnapshot::build(1, &reg, &ts, &tp, 0);
        
        // Build witness from snapshot
        let proof = snap.build_proof(&a).unwrap();
        let wit = WeightWitnessV1 {
            who: a,
            stake_q: snap.stake_q_of(&a),
            trust_q: snap.trust_q_of(&a),
            leaf_index: proof.leaf_index,
            siblings: proof.siblings,
        };
        
        assert!(snap.verify_witness(&wit));
    }

    #[test]
    fn witness_verification_fails_wrong_data() {
        let mut reg = Registry::default();
        let tp = TrustParams {
            alpha_q: ONE_Q,
            beta_q: 0,
            init_q: ONE_Q,
        };
        let mut ts = TrustState::default();
        let a = nid(1);
        reg.insert(a, 100, true);
        ts.set(a, ONE_Q);
        
        let snap = EpochSnapshot::build(1, &reg, &ts, &tp, 0);
        
        // Build witness with wrong stake_q
        let proof = snap.build_proof(&a).unwrap();
        let mut wit = WeightWitnessV1 {
            who: a,
            stake_q: snap.stake_q_of(&a),
            trust_q: snap.trust_q_of(&a),
            leaf_index: proof.leaf_index,
            siblings: proof.siblings,
        };
        
        assert!(snap.verify_witness(&wit));
        
        // Modify stake_q - should fail
        wit.stake_q = 0;
        assert!(!snap.verify_witness(&wit));
    }
}