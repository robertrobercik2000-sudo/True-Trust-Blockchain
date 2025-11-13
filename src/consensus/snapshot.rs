//! Epoch snapshots with Merkle weights tree

use super::types::{NodeId, Registry, StakeQ, ONE_Q};
pub use super::types::Q;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Q32.32 arithmetic helpers
#[inline]
pub fn qmul(a: Q, b: Q) -> Q {
    ((a as u128 * b as u128) >> 32) as Q
}

#[inline]
pub fn qdiv(a: Q, b: Q) -> Option<Q> {
    if b == 0 { return None; }
    Some((((a as u128) << 32) / b as u128) as Q)
}

/// Merkle leaf: hash(who || stake_q || trust_q)
fn merkle_leaf_hash(who: &NodeId, stake_q: StakeQ, trust_q: Q) -> [u8; 32] {
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

/// Merkle parent hash
fn merkle_parent(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"MRK.v1");
    h.update(left);
    h.update(right);
    let out = h.finalize();
    let mut r = [0u8; 32];
    r.copy_from_slice(&out);
    r
}

/// Epoch snapshot with Merkle tree of validator weights
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EpochSnapshot {
    pub epoch: u64,
    pub weights_root: [u8; 32],
    pub weights: HashMap<NodeId, Q>,  // weight = stake_q × trust_q
    pub total_weight_q: u128,
}

impl EpochSnapshot {
    /// Build snapshot from registry + trust state
    pub fn build(epoch: u64, registry: &Registry, trust: &HashMap<NodeId, Q>) -> Self {
        let mut weights = HashMap::new();
        let mut leaves = Vec::new();
        let mut total = 0u128;

        for (node_id, info) in &registry.validators {
            let trust_q = trust.get(node_id).copied().unwrap_or(ONE_Q);
            let weight = qmul(info.stake as Q, trust_q);
            weights.insert(*node_id, weight);
            total += weight as u128;
            
            leaves.push(merkle_leaf_hash(node_id, info.stake, trust_q));
        }

        // Build Merkle tree
        let root = if leaves.is_empty() {
            [0u8; 32]
        } else {
            Self::merkle_root(&leaves)
        };

        Self {
            epoch,
            weights_root: root,
            weights,
            total_weight_q: total,
        }
    }

    fn merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
        if leaves.len() == 1 {
            return leaves[0];
        }

        let mut level: Vec<[u8; 32]> = leaves.to_vec();
        while level.len() > 1 {
            let mut next = Vec::new();
            for chunk in level.chunks(2) {
                if chunk.len() == 2 {
                    next.push(merkle_parent(&chunk[0], &chunk[1]));
                } else {
                    next.push(chunk[0]);
                }
            }
            level = next;
        }
        level[0]
    }

    pub fn get_weight(&self, node_id: &NodeId) -> Option<Q> {
        self.weights.get(node_id).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus::ValidatorInfo;

    #[test]
    fn test_snapshot_build() {
        let mut registry = Registry::new();
        registry.register(ValidatorInfo {
            node_id: [1u8; 32],
            stake: 1000,
            public_key: vec![],
            network_addr: "127.0.0.1:8000".into(),
        });

        let mut trust = HashMap::new();
        trust.insert([1u8; 32], ONE_Q);

        let snap = EpochSnapshot::build(0, &registry, &trust);
        assert_ne!(snap.weights_root, [0u8; 32]);
    }
}
