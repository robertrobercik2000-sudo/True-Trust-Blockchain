#![forbid(unsafe_code)]

use core::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};

// nowa ścieżka: weryfikacja świadka z snapshot.rs (nie rusza starego API)
use crate::snapshot::SnapshotWitnessExt;
use crate::crypto_kmac_consensus::kmac256_hash;

/* ===== Q32.32 ===== */

pub type Q = u64;                // Q32.32
pub const ONE_Q: Q = 1u64 << 32; // 1.0

#[inline]
fn qmul(a: Q, b: Q) -> Q {
    let z = (a as u128) * (b as u128);
    let shifted = z >> 32;
    // Clamp to u64::MAX to prevent overflow
    shifted.min(u64::MAX as u128) as u64
}

#[inline]
fn qadd(a: Q, b: Q) -> Q { a.saturating_add(b) }

#[inline]
fn qdiv(a: Q, b: Q) -> Q {
    if b == 0 { 0 } else {
        let z = (a as u128) << 32;
        (z / (b as u128)).min(u128::from(u64::MAX)) as u64
    }
}

#[inline]
pub fn q_from_ratio(num: u64, den: u64) -> Q {
    ((u128::from(ONE_Q) * u128::from(num)) / u128::from(den.max(1))) as u64
}

#[inline]
pub fn q_from_ratio128(num: u128, den: u128) -> Q {
    if den == 0 { return 0; }
    let z = (u128::from(ONE_Q) * num) / den;
    z.min(u128::from(u64::MAX)) as u64
}

#[inline]
pub fn q_from_basis_points(bp: u32) -> Q {
    ((u128::from(ONE_Q) * u128::from(bp)) / 10_000) as u64
}

#[inline]
fn qclamp01(x: Q) -> Q { x.min(ONE_Q) }

/* ===== Trust ===== */

#[derive(Clone, Copy, Debug)]
pub struct TrustParams { pub alpha_q: Q, pub beta_q: Q, pub init_q: Q }

impl TrustParams {
    /// Create new TrustParams with validation
    pub fn new(alpha_q: Q, beta_q: Q, init_q: Q) -> Result<Self, &'static str> {
        if alpha_q > ONE_Q || beta_q > ONE_Q || init_q > ONE_Q {
            return Err("trust parameters must be <= 1.0");
        }
        Ok(Self { alpha_q, beta_q, init_q })
    }

    #[inline] 
    pub fn decay(&self, t: Q) -> Q { qmul(t, self.alpha_q) }
    
    #[inline] 
    pub fn reward(&self, t: Q) -> Q { qclamp01(qadd(t, self.beta_q)) }
    
    #[inline] 
    pub fn step(&self, t: Q) -> Q { self.reward(self.decay(t)) }
}

pub type NodeId = [u8; 32];

#[derive(Default)]
pub struct TrustState { map: HashMap<NodeId, Q> }

impl TrustState {
    #[inline] 
    pub fn get(&self, id: &NodeId, init: Q) -> Q { 
        *self.map.get(id).unwrap_or(&init) 
    }
    
    #[inline] 
    pub fn set(&mut self, id: NodeId, val: Q) { 
        self.map.insert(id, val); 
    }
    
    #[inline] 
    pub fn apply_block_reward(&mut self, who: &NodeId, p: TrustParams) {
        let t = self.get(who, p.init_q);
        self.set(*who, p.step(t));
    }
}

/* ===== Registry (stake, klucze) ===== */

#[derive(Clone)]
pub struct RegEntry { pub who: NodeId, pub stake: u64, pub active: bool }

#[derive(Default)]
pub struct Registry { pub map: HashMap<NodeId, RegEntry> }

impl Registry {
    pub fn insert(&mut self, who: NodeId, stake: u64, active: bool) {
        self.map.insert(who, RegEntry { who, stake, active });
    }
    
    #[inline] 
    pub fn is_active(&self, who: &NodeId, min_bond: u64) -> bool {
        self.map.get(who).map(|e| e.active && e.stake >= min_bond).unwrap_or(false)
    }
    
    #[inline] 
    pub fn stake(&self, who: &NodeId) -> u64 {
        self.map.get(who).map(|e| e.stake).unwrap_or(0)
    }
    
    #[inline] 
    pub fn stake_mut(&mut self, who: &NodeId) -> Option<&mut u64> {
        self.map.get_mut(who).map(|e| &mut e.stake)
    }
}

/* ===== Snapshot epoki: Σwag + Merkle (z deterministycznym porządkiem) ===== */

pub type StakeQ = Q; // stake_q∈[0,1]

#[derive(Clone, Debug)]
pub struct EpochSnapshot {
    pub epoch: u64,
    pub sum_weights_q: Q,                 // Σ(stake_q * trust_q)
    pub stake_q: HashMap<NodeId, Q>,      // stake_q per id
    pub trust_q_at_snapshot: HashMap<NodeId, Q>, // trust z chwili snapshotu
    pub order: Vec<NodeId>,               // deterministyczny porządek liści (sort po who)
    pub weights_root: [u8; 32],
}

#[derive(Clone, Debug)]
pub struct SnapshotEntry { pub who: NodeId, pub stake_q: StakeQ, pub trust_q: Q }

impl EpochSnapshot {
    pub fn build(epoch: u64, reg: &Registry, trust: &TrustState, tp: &TrustParams, min_bond: u64) -> Self {
        let total: u128 = reg.map.values()
            .filter(|e| e.active && e.stake >= min_bond)
            .map(|e| e.stake as u128)
            .sum();

        let mut entries: Vec<SnapshotEntry> = Vec::new();
        let mut stake_q_map: HashMap<NodeId, Q> = HashMap::new();
        let mut trust_q_map: HashMap<NodeId, Q> = HashMap::new();

        for (who, e) in &reg.map {
            if !(e.active && e.stake >= min_bond) { continue; }
            let sq = if total == 0 { 0 } else { q_from_ratio128(e.stake as u128, total) };
            let tq = trust.get(who, tp.init_q).min(ONE_Q);
            stake_q_map.insert(*who, sq);
            trust_q_map.insert(*who, tq);
            entries.push(SnapshotEntry { who: *who, stake_q: sq, trust_q: tq });
        }

        // deterministyczny porządek
        entries.sort_by(|a, b| a.who.cmp(&b.who));
        let order: Vec<NodeId> = entries.iter().map(|e| e.who).collect();

        // liście w porządku `order`
        let leaves: Vec<[u8; 32]> = entries.iter()
            .map(|e| merkle_leaf_hash(&e.who, e.stake_q, e.trust_q))
            .collect();

        let weights_root = merkle_root(&leaves);
        let sum_weights_q = entries.iter()
            .fold(0u64, |acc, e| acc.saturating_add(qmul(e.stake_q, e.trust_q)));

        Self {
            epoch,
            sum_weights_q,
            stake_q: stake_q_map,
            trust_q_at_snapshot: trust_q_map,
            order,
            weights_root,
        }
    }

    #[inline] 
    pub fn stake_q_of(&self, who: &NodeId) -> Q { 
        *self.stake_q.get(who).unwrap_or(&0) 
    }
    
    #[inline] 
    pub fn trust_q_of(&self, who: &NodeId) -> Q { 
        *self.trust_q_at_snapshot.get(who).unwrap_or(&0) 
    }

    /// Zwraca indeks liścia w `order`
    pub fn leaf_index_of(&self, who: &NodeId) -> Option<u64> {
        self.order.iter().position(|w| w == who).map(|i| i as u64)
    }

    /// Buduje MerkleProof dla `who` z aktualnego snapshotu.
    pub fn build_proof(&self, who: &NodeId) -> Option<MerkleProof> {
        let idx = self.leaf_index_of(who)?;
        // odtwórz liście deterministycznie
        let leaves: Vec<[u8; 32]> = self.order.iter()
            .map(|id| merkle_leaf_hash(id, self.stake_q_of(id), self.trust_q_of(id)))
            .collect();
        Some(merkle_build_proof(&leaves, idx))
    }
}

/* ===== Merkle ===== */

#[derive(Clone, Debug)]
pub struct MerkleProof { pub leaf_index: u64, pub siblings: Vec<[u8; 32]> }

#[inline]
fn merkle_leaf_hash(who: &NodeId, stake_q: StakeQ, trust_q: Q) -> [u8; 32] {
    kmac256_hash(b"WGT.v1", &[
        who,
        &stake_q.to_le_bytes(),
        &trust_q.to_le_bytes(),
    ])
}

#[inline]
fn merkle_parent(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    kmac256_hash(b"MRK.v1", &[a, b])
}

fn merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    if leaves.is_empty() {
        // Special hash for empty tree to avoid collision with [0u8; 32]
        return kmac256_hash(b"MRK.empty.v1", &[]);
    }
    let mut layer = leaves.to_vec();
    while layer.len() > 1 {
        let mut next = Vec::with_capacity((layer.len() + 1) / 2);
        for i in (0..layer.len()).step_by(2) {
            if i + 1 < layer.len() { 
                next.push(merkle_parent(&layer[i], &layer[i + 1])); 
            } else { 
                next.push(merkle_parent(&layer[i], &layer[i])); 
            }
        }
        layer = next;
    }
    layer[0]
}

fn merkle_build_proof(leaves: &[[u8; 32]], leaf_idx: u64) -> MerkleProof {
    let mut idx = leaf_idx as usize;
    let mut layer = leaves.to_vec();
    let mut siblings = Vec::<[u8; 32]>::new();
    while layer.len() > 1 {
        let mut next = Vec::with_capacity((layer.len() + 1) / 2);
        for i in (0..layer.len()).step_by(2) {
            let (a, b) = if i + 1 < layer.len() { 
                (layer[i], layer[i + 1]) 
            } else { 
                (layer[i], layer[i]) 
            };
            // jeśli aktualny liść w parze, zbierz brata
            if i == idx { siblings.push(b); }
            else if i + 1 == idx { siblings.push(a); }
            next.push(merkle_parent(&a, &b));
        }
        idx /= 2;
        layer = next;
    }
    MerkleProof { leaf_index: leaf_idx, siblings }
}

#[allow(unused)]
pub fn verify_merkle(proof: &MerkleProof, leaf: [u8; 32], root: [u8; 32]) -> bool {
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

/* ===== RANDAO beacon ===== */

#[derive(Default, Clone)]
pub struct RandaoEpoch {
    pub commits: HashMap<NodeId, [u8; 32]>,
    pub reveals: HashMap<NodeId, [u8; 32]>,
    pub finalized: bool,
    /// seed używany do sortition w TEJ epoce (snapshot prev_beacon z jej startu)
    pub seed: [u8; 32],
    /// beacon policzony z revealami TEJ epoki – staje się prev_beacon dla epoki+1
    pub beacon: [u8; 32],
}

#[derive(Default)]
pub struct RandaoBeacon {
    pub epochs: HashMap<u64, RandaoEpoch>,
    pub prev_beacon: [u8; 32],
    pub slash_noreveal_bps: u32,
}

impl RandaoBeacon {
    pub fn new(slash_noreveal_bps: u32, genesis_beacon: [u8; 32]) -> Self {
        Self { 
            epochs: HashMap::new(), 
            prev_beacon: genesis_beacon, 
            slash_noreveal_bps 
        }
    }
    
    #[inline]
    pub fn commit_hash(epoch: u64, who: &NodeId, r: &[u8; 32]) -> [u8; 32] {
        kmac256_hash(b"RANDAO.commit.v1", &[
            &epoch.to_le_bytes(),
            who,
            r,
        ])
    }
    
    pub fn commit(&mut self, epoch: u64, who: NodeId, c: [u8; 32]) {
        self.epochs.entry(epoch).or_default().commits.insert(who, c);
    }
    
    pub fn reveal(&mut self, epoch: u64, who: NodeId, r: [u8; 32]) -> bool {
        let e = self.epochs.entry(epoch).or_default();
        match e.commits.get(&who) {
            None => false,
            Some(&c) => {
                if Self::commit_hash(epoch, &who, &r) != c { return false; }
                e.reveals.insert(who, r); 
                true
            }
        }
    }
    
    pub fn finalize_epoch(&mut self, epoch: u64) -> ([u8; 32], Vec<NodeId>) {
        let e = self.epochs.entry(epoch).or_default();
        if e.finalized { return (e.beacon, Vec::new()); }

        // 1) snapshot prev_beacon jako seed TEJ epoki
        let epoch_seed = self.prev_beacon;
        e.seed = epoch_seed;

        // 2) zmiksuj reveale zaczynając od seed
        let mut mix = epoch_seed;
        let revealed: BTreeMap<NodeId, [u8; 32]> = e.reveals.iter().map(|(k, v)| (*k, *v)).collect();
        for (who, r) in &revealed { 
            mix = mix_hash(&mix, who, r); 
        }
        e.beacon = mix;
        e.finalized = true;

        // brakujące
        let mut missing = Vec::new();
        for who in e.commits.keys() { 
            if !e.reveals.contains_key(who) { 
                missing.push(*who); 
            } 
        }

        // 3) beacon staje się prev_beacon dla następnej epoki
        self.prev_beacon = e.beacon;
        (e.beacon, missing)
    }
    
    #[inline]
    pub fn value(&self, epoch: u64, slot: u64) -> [u8; 32] {
        // stabilny seed dla danej epoki
        let base = match self.epochs.get(&epoch) {
            Some(e) if e.finalized && e.seed != [0u8; 32] => e.seed,
            Some(e) if !e.finalized => e.seed, // Use seed even if not finalized
            _ => self.prev_beacon,
        };
        kmac256_hash(b"RANDAO.slot.v1", &[
            &epoch.to_le_bytes(),
            &slot.to_le_bytes(),
            &base,
        ])
    }
}

#[inline]
fn mix_hash(prev: &[u8; 32], who: &NodeId, r: &[u8; 32]) -> [u8; 32] {
    kmac256_hash(b"RANDAO.mix.v1", &[prev, who, r])
}

/* ===== Sortition (BEACON+MERKLE) ===== */

#[derive(Clone, Debug)]
pub struct LeaderWitness {
    pub who: NodeId,
    pub slot: u64,
    pub epoch: u64,
    pub weights_root: [u8; 32],
    pub weight_proof: MerkleProof,
    pub stake_q: StakeQ,
    pub trust_q: Q,
}

#[inline]
fn elig_hash(beacon: &[u8; 32], slot: u64, who: &NodeId) -> u64 {
    let hash = kmac256_hash(b"ELIG.v1", &[
        beacon,
        &slot.to_le_bytes(),
        who,
    ]);
    // Use first 8 bytes of hash output
    let mut w = [0u8; 8];
    w.copy_from_slice(&hash[..8]);
    u64::from_be_bytes(w)
}

#[inline]
fn bound_u64(p_q: Q) -> u64 {
    (((p_q as u128) << 32).min(u128::from(u64::MAX))) as u64
}

#[inline]
fn prob_threshold_q(lambda_q: Q, stake_q: Q, trust_q: Q, sum_weights_q: Q) -> Q {
    // Ensure minimum sum_weights_q to avoid division issues
    let sum = sum_weights_q.max(ONE_Q / 1_000_000); // Minimum 0.000001
    let wi = qmul(stake_q, qclamp01(trust_q));
    qclamp01(qmul(lambda_q, qdiv(wi, sum)))
}

#[derive(Clone, Copy, Debug)]
pub struct PotParams { 
    pub trust: TrustParams, 
    pub lambda_q: Q, 
    pub min_bond: u64, 
    pub slash_noreveal_bps: u32 
}

/// Common verification logic extracted to reduce duplication
fn verify_leader_common(
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

    let p_q = prob_threshold_q(params.lambda_q, stake_q, trust_q, epoch_snap.sum_weights_q);
    let b = beacon.value(epoch, slot);
    let y = elig_hash(&b, slot, who);
    if y > bound_u64(p_q) { return None; }

    let denom = u128::from(y).saturating_add(1);
    let weight = (u128::from(u64::MAX) + 1) / denom;
    Some(weight)
}

pub fn verify_leader_and_update_trust(
    reg: &Registry,
    epoch_snap: &EpochSnapshot,
    beacon: &RandaoBeacon,
    trust_state: &mut TrustState,
    params: &PotParams,
    wit: &LeaderWitness,
) -> Option<u128> {
    // Verify Merkle proof
    if wit.weights_root != epoch_snap.weights_root { return None; }
    let leaf = merkle_leaf_hash(&wit.who, wit.stake_q, wit.trust_q);
    if !verify_merkle(&wit.weight_proof, leaf, epoch_snap.weights_root) { 
        return None; 
    }

    // Common verification
    let weight = verify_leader_common(
        reg, epoch_snap, beacon, params, 
        wit.epoch, wit.slot, &wit.who, wit.stake_q, wit.trust_q
    )?;

    // Update trust
    trust_state.apply_block_reward(&wit.who, params.trust);
    Some(weight)
}

// === Nowa ścieżka: weryfikacja lidera na podstawie kompaktowego świadka (snapshot.rs) ===
pub fn verify_leader_with_witness(
    reg: &Registry,
    epoch_snap: &EpochSnapshot,
    beacon: &RandaoBeacon,
    trust_state: &mut TrustState,
    params: &PotParams,
    epoch: u64,
    slot: u64,
    wit: &crate::snapshot::WeightWitnessV1,
) -> Option<u128> {
    // Zweryfikuj kompaktowy świadek względem root w snapshotcie
    if !epoch_snap.verify_witness(wit) { return None; }

    // Common verification
    let weight = verify_leader_common(
        reg, epoch_snap, beacon, params, 
        epoch, slot, &wit.who, wit.stake_q, wit.trust_q
    )?;

    // Update trust
    trust_state.apply_block_reward(&wit.who, params.trust);
    Some(weight)
}

/* ===== Equivocation ===== */

#[derive(Clone, Copy)]
pub struct Proposal { pub who: NodeId, pub slot: u64, pub header_hash: [u8; 32] }

pub fn detect_equivocation(proposals: &[Proposal]) -> bool {
    if proposals.is_empty() { return false; }
    
    // Check that all proposals are from the same node and slot
    let slot = proposals[0].slot; 
    let who = proposals[0].who;
    
    for p in proposals.iter().skip(1) {
        if p.slot != slot || p.who != who { 
            return false; // Different node or slot - not equivocation
        }
    }
    
    // Check for different header hashes
    let mut set = HashSet::<[u8; 32]>::new();
    for p in proposals {
        set.insert(p.header_hash);
    }
    set.len() > 1
}

pub fn slash_equivocation(
    reg: &mut Registry, 
    trust: &mut TrustState, 
    who: &NodeId, 
    tp: TrustParams, 
    penalty_bps: u32
) {
    trust.set(*who, tp.init_q);
    if let Some(st) = reg.stake_mut(who) { 
        *st = slash_bps(*st, penalty_bps); 
    }
}

#[inline]
fn slash_bps(stake: u64, bps: u32) -> u64 {
    // Validate bps <= 10000 (100%)
    let bps = bps.min(10000);
    let cut = ((stake as u128) * (bps as u128)) / 10_000u128;
    let cut = cut.min(stake as u128) as u64; // Ensure cut <= stake
    stake.saturating_sub(cut)
}

/* ===== Integracja RANDAO z ekonomią (kary za brak reveal) ===== */

pub fn finalize_epoch_and_slash(
    beacon: &mut RandaoBeacon,
    epoch: u64,
    registry: &mut Registry,
    trust: &mut TrustState,
    tp: TrustParams,
) -> [u8; 32] {
    // NOTE: This function modifies multiple state objects.
    // Ensure proper synchronization if called from multiple threads.
    let (val, missing) = beacon.finalize_epoch(epoch);
    for who in missing {
        if let Some(st) = registry.stake_mut(&who) { 
            *st = slash_bps(*st, beacon.slash_noreveal_bps); 
        }
        trust.set(who, tp.init_q);
    }
    val
}

/* ===== Test helpers (opcjonalnie) ===== */

#[cfg(test)]
mod tests {
    use super::*;
    
    fn nid(n: u8) -> NodeId { 
        let mut id = [0u8; 32]; 
        id[0] = n; 
        id 
    }

    #[test]
    fn prob_monotone() {
        let p1 = super::prob_threshold_q(
            q_from_ratio(11,10), 
            q_from_ratio(1,10), 
            q_from_ratio(1,1), 
            q_from_ratio(1,1)
        );
        let p2 = super::prob_threshold_q(
            q_from_ratio(11,10), 
            q_from_ratio(2,10), 
            q_from_ratio(1,1), 
            q_from_ratio(1,1)
        );
        assert!(p2 >= p1);
    }

    #[test]
    fn randao_commit_reveal() {
        let mut b = RandaoBeacon::new(500, [7u8; 32]);
        let e = 1u64; 
        let who = nid(1);
        let r = [9u8; 32]; 
        let c = RandaoBeacon::commit_hash(e, &who, &r);
        b.commit(e, who, c);
        assert!(b.reveal(e, who, r));
        let (val, miss) = b.finalize_epoch(e);
        assert_eq!(miss.len(), 0); 
        assert_ne!(val, [0u8; 32]);
        let v0 = b.value(e, 0);
        let exp = kmac256_hash(b"RANDAO.slot.v1", &[
            &e.to_le_bytes(),
            &0u64.to_le_bytes(),
            &val,
        ]);
        assert_eq!(v0, exp);
    }

    #[test]
    fn snapshot_deterministic_root() {
        let mut reg = Registry::default();
        let tp = TrustParams { 
            alpha_q: q_from_basis_points(9900), 
            beta_q: q_from_basis_points(100), 
            init_q: q_from_basis_points(1000) 
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
        let s1 = EpochSnapshot::build(1, &reg, &ts, &tp, 0);
        let s2 = EpochSnapshot::build(1, &reg, &ts, &tp, 0);
        assert_eq!(s1.weights_root, s2.weights_root);
        // proof działa
        let p = s1.build_proof(&a).unwrap();
        let leaf = super::merkle_leaf_hash(&a, s1.stake_q_of(&a), s1.trust_q_of(&a));
        assert!(verify_merkle(&p, leaf, s1.weights_root));
    }

    #[test]
    fn weights_ratio_uses_u128_den() {
        let mut reg = Registry::default();
        let tp = TrustParams { 
            alpha_q: ONE_Q, 
            beta_q: 0, 
            init_q: ONE_Q 
        };
        let mut ts = TrustState::default();
        let a = [1u8; 32]; 
        let b = [2u8; 32];
        reg.insert(a, u64::MAX, true);
        reg.insert(b, u64::MAX - 1, true);
        ts.set(a, ONE_Q); 
        ts.set(b, ONE_Q);
        let s = EpochSnapshot::build(1, &reg, &ts, &tp, 0);
        assert!(s.stake_q_of(&a) > s.stake_q_of(&b));
    }

    #[test]
    fn randao_value_stable_before_after_finalize() {
        let mut b = RandaoBeacon::new(0, [7u8; 32]);
        let e = 42u64;
        // wartość na starcie epoki (przed finalize)
        let v_pre = b.value(e, 123);
        // commit/reveal + finalize
        let who = [8u8; 32];
        let r = [9u8; 32];
        let c = RandaoBeacon::commit_hash(e, &who, &r);
        b.commit(e, who, c);
        assert!(b.reveal(e, who, r));
        let _ = b.finalize_epoch(e);
        let v_post = b.value(e, 123);
        assert_eq!(v_pre, v_post, "sortition seed must be stable across finalize");
    }

    #[test]
    fn empty_merkle_root() {
        let root = super::merkle_root(&[]);
        assert_ne!(root, [0u8; 32], "empty root should not be zero");
    }

    #[test]
    fn slash_bps_validation() {
        assert_eq!(super::slash_bps(1000, 5000), 500); // 50%
        assert_eq!(super::slash_bps(1000, 10000), 0); // 100%
        assert_eq!(super::slash_bps(1000, 15000), 0); // Clamped to 100%
    }

    #[test]
    fn detect_equivocation_cases() {
        let who = nid(1);
        let slot = 42u64;
        let hash1 = [1u8; 32];
        let hash2 = [2u8; 32];
        
        // Same node, same slot, different hashes = equivocation
        let proposals = vec![
            Proposal { who, slot, header_hash: hash1 },
            Proposal { who, slot, header_hash: hash2 },
        ];
        assert!(super::detect_equivocation(&proposals));
        
        // Different nodes = not equivocation
        let proposals2 = vec![
            Proposal { who: nid(1), slot, header_hash: hash1 },
            Proposal { who: nid(2), slot, header_hash: hash2 },
        ];
        assert!(!super::detect_equivocation(&proposals2));
        
        // Same hash = not equivocation
        let proposals3 = vec![
            Proposal { who, slot, header_hash: hash1 },
            Proposal { who, slot, header_hash: hash1 },
        ];
        assert!(!super::detect_equivocation(&proposals3));
    }
}
