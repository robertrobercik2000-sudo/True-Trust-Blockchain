// Snapshot witness structures and verification
#![allow(dead_code)]

use super::consensus::{NodeId, StakeQ, Q, merkle_leaf_hash};
use sha2::{Digest, Sha256};

/// Kompaktowy świadek wagi węzła w snapshotcie epoki
#[derive(Clone, Debug)]
pub struct WeightWitnessV1 {
    pub who: NodeId,
    pub stake_q: StakeQ,
    pub trust_q: Q,
    pub leaf_index: u64,
    pub siblings: Vec<[u8; 32]>,
}

impl WeightWitnessV1 {
    /// Tworzy nowy świadek z pełnych danych
    pub fn new(
        who: NodeId,
        stake_q: StakeQ,
        trust_q: Q,
        leaf_index: u64,
        siblings: Vec<[u8; 32]>,
    ) -> Self {
        Self { who, stake_q, trust_q, leaf_index, siblings }
    }

    /// Weryfikuje świadek względem danego korzenia Merkle
    pub fn verify_against_root(&self, root: &[u8; 32]) -> bool {
        let leaf = merkle_leaf_hash(&self.who, self.stake_q, self.trust_q);
        let mut acc = leaf;
        let mut idx = self.leaf_index;
        
        for sib in &self.siblings {
            acc = if (idx & 1) == 0 {
                merkle_parent(&acc, sib)
            } else {
                merkle_parent(sib, &acc)
            };
            idx >>= 1;
        }
        
        acc == *root
    }
}

/// Helper: hash rodzica w drzewie Merkle (musi być zgodny z consensus.rs)
#[inline]
fn merkle_parent(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"MRK.v1");
    h.update(a);
    h.update(b);
    let out = h.finalize();
    let mut r = [0u8; 32];
    r.copy_from_slice(&out);
    r
}

/// Trait rozszerzający EpochSnapshot o weryfikację kompaktowych świadków
pub trait SnapshotWitnessExt {
    /// Weryfikuje WeightWitnessV1 względem tego snapshotu
    fn verify_witness(&self, wit: &WeightWitnessV1) -> bool;
    
    /// Buduje WeightWitnessV1 dla danego węzła
    fn build_witness(&self, who: &NodeId) -> Option<WeightWitnessV1>;
}

// Implementacja dla EpochSnapshot z consensus.rs
impl SnapshotWitnessExt for super::consensus::EpochSnapshot {
    fn verify_witness(&self, wit: &WeightWitnessV1) -> bool {
        // Sprawdź czy stake_q i trust_q się zgadzają
        if wit.stake_q != self.stake_q_of(&wit.who) {
            return false;
        }
        if wit.trust_q != self.trust_q_of(&wit.who) {
            return false;
        }
        
        // Sprawdź czy leaf_index jest poprawny
        match self.leaf_index_of(&wit.who) {
            Some(idx) if idx == wit.leaf_index => {}
            _ => return false,
        }
        
        // Weryfikuj Merkle proof
        wit.verify_against_root(&self.weights_root)
    }
    
    fn build_witness(&self, who: &NodeId) -> Option<WeightWitnessV1> {
        let stake_q = self.stake_q_of(who);
        let trust_q = self.trust_q_of(who);
        let proof = self.build_proof(who)?;
        
        Some(WeightWitnessV1::new(
            *who,
            stake_q,
            trust_q,
            proof.leaf_index,
            proof.siblings,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus::*;

    fn nid(n: u8) -> NodeId {
        let mut id = [0u8; 32];
        id[0] = n;
        id
    }

    #[test]
    fn witness_roundtrip() {
        let mut reg = Registry::default();
        let tp = TrustParams {
            alpha_q: q_from_basis_points(9900),
            beta_q: q_from_basis_points(100),
            init_q: q_from_basis_points(1000),
        };
        let mut ts = TrustState::default();
        
        let a = nid(1);
        let b = nid(2);
        let c = nid(3);
        
        reg.insert(a, 100, true);
        reg.insert(b, 50, true);
        reg.insert(c, 150, true);
        
        ts.set(a, q_from_basis_points(5000));
        ts.set(b, q_from_basis_points(9000));
        ts.set(c, q_from_basis_points(1000));
        
        let snap = EpochSnapshot::build(1, &reg, &ts, &tp, 0);
        
        // Zbuduj świadek dla węzła 'a'
        let wit = snap.build_witness(&a).expect("witness should exist");
        
        // Weryfikuj świadek
        assert!(snap.verify_witness(&wit), "witness verification should pass");
        
        // Modyfikuj świadek i sprawdź że weryfikacja zawiedzie
        let mut bad_wit = wit.clone();
        bad_wit.stake_q = bad_wit.stake_q.wrapping_add(1);
        assert!(!snap.verify_witness(&bad_wit), "modified witness should fail");
    }

    #[test]
    fn witness_wrong_root() {
        let mut reg = Registry::default();
        let tp = TrustParams {
            alpha_q: ONE_Q,
            beta_q: 0,
            init_q: ONE_Q,
        };
        let ts = TrustState::default();
        
        let a = nid(1);
        reg.insert(a, 100, true);
        
        let snap = EpochSnapshot::build(1, &reg, &ts, &tp, 0);
        let wit = snap.build_witness(&a).expect("witness should exist");
        
        // Weryfikacja z poprawnym rootem
        assert!(wit.verify_against_root(&snap.weights_root));
        
        // Weryfikacja z błędnym rootem
        let bad_root = [0xffu8; 32];
        assert!(!wit.verify_against_root(&bad_root));
    }
}
