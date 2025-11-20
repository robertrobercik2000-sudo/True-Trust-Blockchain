#![forbid(unsafe_code)]

use std::collections::HashMap;

use sha2::{Digest, Sha256};

use crate::node_id::NodeId;
use crate::rtt_pro::Q;
use crate::consensus_weights::{compute_final_weight_q, Weight};
use crate::consensus_pro::{ConsensusPro, Slot};

/// Merkle proof dla pojedynczego walidatora.
#[derive(Clone, Debug)]
pub struct MerkleProof {
    pub leaf_index: u64,
    pub siblings: Vec<[u8; 32]>,
}

/// Snapshot jednej pozycji (walidatora) w danej epoce.
#[derive(Clone, Debug)]
pub struct ValidatorSnapshot {
    pub who: NodeId,
    pub stake_q: Q,
    pub trust_q: Q,
    pub quality_q: Q,
    pub weight: Weight,
}

/// Snapshot wag dla całej sieci w danej epoce.
#[derive(Clone, Debug)]
pub struct EpochSnapshot {
    pub epoch: Slot,
    pub weights_root: [u8; 32],

    validators: Vec<ValidatorSnapshot>,
    index: HashMap<NodeId, u64>,
}

impl EpochSnapshot {
    /// Zbuduj snapshot z bieżącego stanu `ConsensusPro`.
    pub fn build(epoch: Slot, cons: &ConsensusPro) -> Self {
        // 1. Zrzut walidatorów
        let mut vals: Vec<ValidatorSnapshot> = cons
            .validators_iter()
            .map(|(_, v)| {
                let w = compute_final_weight_q(v.trust_q, v.quality_q, v.stake_q);
                ValidatorSnapshot {
                    who: v.id,
                    stake_q: v.stake_q,
                    trust_q: v.trust_q,
                    quality_q: v.quality_q,
                    weight: w,
                }
            })
            .collect();

        // 2. Deterministyczne uporządkowanie (po NodeId)
        vals.sort_by(|a, b| a.who.cmp(&b.who));

        // 3. Indeks NodeId → leaf_index
        let mut index = HashMap::with_capacity(vals.len());
        for (i, v) in vals.iter().enumerate() {
            index.insert(v.who, i as u64);
        }

        // 4. Merkle root
        let leaves: Vec<[u8; 32]> = vals.iter().map(merkle_leaf_hash).collect();
        let weights_root = merkle_root(&leaves);

        Self {
            epoch,
            weights_root,
            validators: vals,
            index,
        }
    }

    pub fn stake_q_of(&self, who: &NodeId) -> Q {
        self.validators
            .get(self.index.get(who).copied().unwrap_or(u64::MAX) as usize)
            .map(|v| v.stake_q)
            .unwrap_or(0)
    }

    pub fn trust_q_of(&self, who: &NodeId) -> Q {
        self.validators
            .get(self.index.get(who).copied().unwrap_or(u64::MAX) as usize)
            .map(|v| v.trust_q)
            .unwrap_or(0)
    }

    pub fn quality_q_of(&self, who: &NodeId) -> Q {
        self.validators
            .get(self.index.get(who).copied().unwrap_or(u64::MAX) as usize)
            .map(|v| v.quality_q)
            .unwrap_or(0)
    }

    pub fn weight_of(&self, who: &NodeId) -> Weight {
        self.validators
            .get(self.index.get(who).copied().unwrap_or(u64::MAX) as usize)
            .map(|v| v.weight)
            .unwrap_or(0)
    }

    pub fn leaf_index_of(&self, who: &NodeId) -> Option<u64> {
        self.index.get(who).copied()
    }

    /// Zbuduj Merkle proof dla `who`.
    pub fn build_proof(&self, who: &NodeId) -> Option<MerkleProof> {
        let idx = self.index.get(who).copied()? as usize;
        let leaves: Vec<[u8; 32]> = self.validators.iter().map(merkle_leaf_hash).collect();
        let siblings = merkle_siblings(&leaves, idx)?;
        Some(MerkleProof {
            leaf_index: idx as u64,
            siblings,
        })
    }
}

/// Dane, które light-client trzyma w świadectwie.
#[derive(Clone, Debug)]
pub struct WeightWitnessV1 {
    pub who: NodeId,
    pub stake_q: Q,
    pub trust_q: Q,
    pub quality_q: Q,
    pub weight: Weight,
    pub leaf_index: u64,
    pub siblings: Vec<[u8; 32]>,
}

/// Weryfikacja świadka względem snapshotu PRO.
pub trait SnapshotWitnessExt {
    fn verify_witness(&self, wit: &WeightWitnessV1) -> bool;
}

impl SnapshotWitnessExt for EpochSnapshot {
    fn verify_witness(&self, wit: &WeightWitnessV1) -> bool {
        // 1. dane muszą się zgadzać z snapshotem
        if self.stake_q_of(&wit.who) != wit.stake_q {
            return false;
        }
        if self.trust_q_of(&wit.who) != wit.trust_q {
            return false;
        }
        if self.quality_q_of(&wit.who) != wit.quality_q {
            return false;
        }
        if self.weight_of(&wit.who) != wit.weight {
            return false;
        }

        // 2. indeks liścia musi się zgadzać
        match self.leaf_index_of(&wit.who) {
            Some(idx) if idx == wit.leaf_index => {}
            _ => return false,
        }

        // 3. weryfikacja Merkle
        let leaf = merkle_leaf_hash_full(
            &wit.who,
            wit.stake_q,
            wit.trust_q,
            wit.quality_q,
            wit.weight,
        );
        let proof = MerkleProof {
            leaf_index: wit.leaf_index,
            siblings: wit.siblings.clone(),
        };
        verify_merkle(&proof, leaf, self.weights_root)
    }
}

// ============ Merkle helpers (SHA-256, PQ-safe symetrycznie) ============

fn merkle_leaf_hash(v: &ValidatorSnapshot) -> [u8; 32] {
    merkle_leaf_hash_full(&v.who, v.stake_q, v.trust_q, v.quality_q, v.weight)
}

fn merkle_leaf_hash_full(
    who: &NodeId,
    stake_q: Q,
    trust_q: Q,
    quality_q: Q,
    weight: Weight,
) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"WGT-PRO.v1");
    h.update(who);
    h.update(stake_q.to_le_bytes());
    h.update(trust_q.to_le_bytes());
    h.update(quality_q.to_le_bytes());
    h.update(weight.to_be_bytes());
    let out = h.finalize();
    let mut r = [0u8; 32];
    r.copy_from_slice(&out);
    r
}

fn merkle_parent(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"MRK-PRO.v1");
    h.update(a);
    h.update(b);
    let out = h.finalize();
    let mut r = [0u8; 32];
    r.copy_from_slice(&out);
    r
}

fn merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    if leaves.is_empty() {
        return [0u8; 32];
    }
    let mut layer = leaves.to_vec();
    while layer.len() > 1 {
        let mut next = Vec::with_capacity((layer.len() + 1) / 2);
        for chunk in layer.chunks(2) {
            let left = chunk[0];
            let right = if chunk.len() == 2 { chunk[1] } else { chunk[0] };
            next.push(merkle_parent(&left, &right));
        }
        layer = next;
    }
    layer[0]
}

fn merkle_siblings(leaves: &[[u8; 32]], index: usize) -> Option<Vec<[u8; 32]>> {
    if leaves.is_empty() || index >= leaves.len() {
        return None;
    }
    let mut siblings = Vec::new();
    let mut idx = index;
    let mut layer = leaves.to_vec();

    while layer.len() > 1 {
        let sib_idx = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
        let sib = layer.get(sib_idx).copied().unwrap_or(layer[idx]);
        siblings.push(sib);

        // budujemy następną warstwę
        let mut next = Vec::with_capacity((layer.len() + 1) / 2);
        for chunk in layer.chunks(2) {
            let left = chunk[0];
            let right = if chunk.len() == 2 { chunk[1] } else { chunk[0] };
            next.push(merkle_parent(&left, &right));
        }
        layer = next;
        idx /= 2;
    }

    Some(siblings)
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
