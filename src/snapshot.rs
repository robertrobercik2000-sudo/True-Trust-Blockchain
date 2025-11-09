#![forbid(unsafe_code)]

//! Snapshot & Witness — epokowe wagi + kompaktowy dowód Merkle (ABI, encode/decode, weryfikacja)
//! ==============================================================================================
//! Ten moduł jest **kanonicznym** miejscem na cały mechanizm snapshotu wag oraz świadków wag
//! kompatybilnych z PoT v3 (RANDAO + BEACON+MERKLE). Został **utwardzony** pod hybrydową finalność
//! (TMF 2/3 ∧ PoS‑guard 1/3), ale zachowuje wsteczną kompatybilność z `consensus.rs`.
//!
//! Co dostajesz:
//! - `WeightWitnessV1` — spójny, binarny ABI świadka: (who, stake_q, trust_q, leaf_index, siblings[32B]*),
//!   z `encode()/decode()` i weryfikacją wobec `weights_root`.
//! - Funkcje Merkle (WGT.v1/MRK.v1) **identyczne** jak w `consensus.rs` (możesz używać których wolisz).
//! - Helpery do pracy z `EpochSnapshot`: budowa świadka, weryfikacja, oraz **nowe**: `sum_trust_q()`.
//! - Dodatkowe sanity‑checki (clamp trust ≤ 1.0, zgodność `leaf_index` z porządkiem `order`).
//!
//! Uwaga integracyjna:
//! - `consensus::verify_leader_and_update_trust(...)` nadal przyjmuje stary `MerkleProof`.
//!   Jeśli chcesz używać kompaktowego ABI świadka, dodaj cienką warstwę adaptacyjną po stronie call‑site:
//!   wywołaj `snapshot.verify_witness(&wit)` a następnie przekaż `wit.stake_q`, `wit.trust_q`, `wit.leaf_index`
//!   i `snapshot.weights_root` do weryfikacji eligibility (nie musisz już składać `MerkleProof`).

use sha2::{Digest, Sha256};

use crate::consensus::{Q, ONE_Q, NodeId, EpochSnapshot};

pub type Hash32 = [u8;32];

/* ===== Merkle kompatybilny z consensus.rs ===== */

#[inline]
pub fn merkle_leaf_hash(who: &NodeId, stake_q: Q, trust_q: Q) -> Hash32 {
    let mut h = Sha256::new();
    h.update(b"WGT.v1");
    h.update(who);
    h.update(stake_q.to_le_bytes());
    h.update(trust_q.to_le_bytes());
    let out = h.finalize();
    let mut r=[0u8;32]; r.copy_from_slice(&out); r
}

#[inline]
pub fn merkle_parent(a:&Hash32, b:&Hash32) -> Hash32 {
    let mut h = Sha256::new();
    h.update(b"MRK.v1"); h.update(a); h.update(b);
    let out=h.finalize(); let mut r=[0u8;32]; r.copy_from_slice(&out); r
}

fn merkle_build_siblings_in_order(leaves:&[Hash32], leaf_idx:usize) -> Vec<Hash32> {
    let mut idx = leaf_idx;
    let mut layer = leaves.to_vec();
    let mut siblings = Vec::<Hash32>::new();
    while layer.len() > 1 {
        let mut next = Vec::with_capacity((layer.len()+1)/2);
        for i in (0..layer.len()).step_by(2) {
            let left  = layer[i];
            let right = if i+1 < layer.len() { layer[i+1] } else { layer[i] };
            if i == idx || i+1 == idx { siblings.push(if i==idx { right } else { left }); idx = i/2; }
            next.push(merkle_parent(&left,&right));
        }
        layer = next;
    }
    siblings
}

#[inline]
pub fn merkle_verify_from_siblings(leaf:Hash32, leaf_index:u64, siblings:&[Hash32], root:&Hash32) -> bool {
    let mut acc = leaf; let mut idx = leaf_index;
    for sib in siblings { acc = if (idx & 1)==0 { merkle_parent(&acc, sib) } else { merkle_parent(sib, &acc) }; idx >>= 1; }
    &acc == root
}

/* ===== WeightWitnessV1 (kanoniczne ABI) ===== */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeightWitnessV1 {
    /// Tożsamość walidatora — **musi** odpowiadać pozycji w `EpochSnapshot.order` pod `leaf_index`.
    pub who: NodeId,
    /// Znormalizowany stake (Q32.32) z chwili snapshotu.
    pub stake_q: Q,
    /// Znormalizowany trust (Q32.32, clamp ≤ 1.0) z chwili snapshotu.
    pub trust_q: Q,
    /// Pozycja liścia w uporządkowanym snapshotcie (`order`: sort rosnąco po who).
    pub leaf_index: u64,
    /// Hash'e rodzeństw (lewa/prawa strona zgodnie z `merkle_verify_from_siblings`).
    pub siblings: Vec<Hash32>,
}

impl WeightWitnessV1 {
    pub const VERSION: u8 = 1;
    pub const DOMAIN: &'static [u8] = b"WGT.witness.v1"; // do ewentualnych podpisów

    /// Kanoniczne kodowanie (LE dla liczb):
    /// V[1] | who[32] | stake_q[u64] | trust_q[u64] | leaf_index[u64] | sib_count[u8] | siblings[sib_count*32]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(1 + 32 + 8 + 8 + 8 + 1 + self.siblings.len()*32);
        out.push(Self::VERSION);
        out.extend_from_slice(&self.who);
        out.extend_from_slice(&self.stake_q.to_le_bytes());
        out.extend_from_slice(&self.trust_q.to_le_bytes());
        out.extend_from_slice(&self.leaf_index.to_le_bytes());
        out.push(self.siblings.len() as u8);
        for s in &self.siblings { out.extend_from_slice(s); }
        out
    }

    pub fn decode(mut bytes:&[u8]) -> Option<Self> {
        if bytes.len() < 1+32+8+8+8+1 { return None; }
        if bytes[0] != Self::VERSION { return None; }
        bytes = &bytes[1..];
        let mut who=[0u8;32]; who.copy_from_slice(&bytes[..32]); bytes=&bytes[32..];
        let mut u8_8 = [0u8;8];
        u8_8.copy_from_slice(&bytes[..8]); let stake_q = u64::from_le_bytes(u8_8); bytes=&bytes[8..];
        u8_8.copy_from_slice(&bytes[..8]); let trust_q = u64::from_le_bytes(u8_8); bytes=&bytes[8..];
        u8_8.copy_from_slice(&bytes[..8]); let leaf_index = u64::from_le_bytes(u8_8); bytes=&bytes[8..];
        let sib_cnt = bytes[0] as usize; bytes=&bytes[1..];
        if bytes.len() < sib_cnt*32 { return None; }
        let mut siblings = Vec::with_capacity(sib_cnt);
        for i in 0..sib_cnt {
            let mut h=[0u8;32];
            h.copy_from_slice(&bytes[i*32..i*32+32]);
            siblings.push(h);
        }
        Some(Self{ who, stake_q, trust_q, leaf_index, siblings })
    }
}

/* ===== Rozszerzenia na EpochSnapshot ===== */

pub trait SnapshotWitnessExt {
    /// Zbuduj kompaktowy świadek wag dla `who`.
    fn build_compact_witness(&self, who:&NodeId) -> Option<WeightWitnessV1>;
    /// Zweryfikuj świadka względem `weights_root` snapshotu.
    fn verify_witness(&self, wit:&WeightWitnessV1) -> bool;
    /// Suma zaufania (Q32.32) w snapshotcie — przydatna do TMF/hybryd.
    fn sum_trust_q(&self) -> Q;
}

impl SnapshotWitnessExt for EpochSnapshot {
    fn build_compact_witness(&self, who:&NodeId) -> Option<WeightWitnessV1> {
        let leaf_idx = self.order.iter().position(|w| w == who)?;
        let leaves: Vec<Hash32> = self.order.iter()
            .map(|id| merkle_leaf_hash(id, self.stake_q_of(id), self.trust_q_of(id)))
            .collect();
        let siblings = merkle_build_siblings_in_order(&leaves, leaf_idx);
        Some(WeightWitnessV1{
            who:*who,
            stake_q:self.stake_q_of(who),
            trust_q:self.trust_q_of(who),
            leaf_index: leaf_idx as u64,
            siblings,
        })
    }

    fn verify_witness(&self, wit:&WeightWitnessV1) -> bool {
        // 1) who musi zgadzać się z `order[leaf_index]`
        if self.order.get(wit.leaf_index as usize).copied().unwrap_or([0;32]) != wit.who { return false; }
        // 2) trust w ABI musi być „clampowany" ≤ 1.0
        if wit.trust_q > ONE_Q { return false; }
        // 3) liść (who, stake_q, trust_q) i ścieżka → root
        let leaf = merkle_leaf_hash(&wit.who, wit.stake_q, wit.trust_q);
        merkle_verify_from_siblings(leaf, wit.leaf_index, &wit.siblings, &self.weights_root)
    }

    fn sum_trust_q(&self) -> Q {
        let mut s: Q = 0;
        for who in &self.order { s = s.saturating_add(self.trust_q_of(who)); }
        s
    }
}

/* ===== Testy ===== */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus::{Registry, TrustParams, TrustState, q_from_basis_points};

    fn nid(n:u8)->NodeId{ let mut id=[0u8;32]; id[0]=n; id }

    #[test]
    fn witness_roundtrip_and_verify() {
        let mut reg = Registry::default();
        let mut ts  = TrustState::default();
        let tp = TrustParams{ alpha_q:q_from_basis_points(9900), beta_q:q_from_basis_points(100), init_q:q_from_basis_points(0) };
        reg.insert(nid(1), 100, true);
        reg.insert(nid(2), 50, true);
        ts.set(nid(1), q_from_basis_points(5000));
        ts.set(nid(2), q_from_basis_points(8000));
        let snap = crate::consensus::EpochSnapshot::build(1, &reg, &ts, &tp, 0);
        let who = nid(1);
        let wit = snap.build_compact_witness(&who).expect("witness");
        assert!(snap.verify_witness(&wit));
        // encode/decode
        let enc = wit.encode();
        let dec = WeightWitnessV1::decode(&enc).expect("decode");
        assert_eq!(wit, dec);
    }

    #[test]
    fn sum_trust_q_is_consistent() {
        let mut reg = Registry::default();
        let mut ts  = TrustState::default();
        let tp = TrustParams{ alpha_q:q_from_basis_points(9900), beta_q:q_from_basis_points(100), init_q:q_from_basis_points(0) };
        for i in 1..=4 { reg.insert(nid(i), 100, true); ts.set(nid(i), q_from_basis_points(1000 * i as u32)); }
        let snap = crate::consensus::EpochSnapshot::build(1, &reg, &ts, &tp, 0);
        let mut manual: Q = 0; for who in &snap.order { manual = manual.saturating_add(snap.trust_q_of(who)); }
        assert_eq!(manual, snap.sum_trust_q());
    }
}
