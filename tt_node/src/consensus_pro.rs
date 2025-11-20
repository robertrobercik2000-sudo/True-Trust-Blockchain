#![forbid(unsafe_code)]

//! TRUE_TRUST – Consensus PRO (PQ-only, deterministic)
//!
//! This module combines:
//! - RTT PRO (`crate::rtt_pro`) – trust T(v) ∈ [0,1] (Q32.32),
//! - "Golden Trio" quality (Q(v) ∈ [0,1] Q32.32 – fed from execution layer),
//! - stake (S(v) – normalized to [0,1] Q32.32),
//! - deterministic weights (`crate::consensus_weights`).
//!
//! Zero `f64` in consensus path (leader selection, fork-choice). F64
//! can only be used in *_debug functions that aren't on hot-path.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::node_id::NodeId;
use crate::rtt_pro::{q_from_f64, q_to_f64, Q, TrustGraph, TrustScore, RTTConfig, ONE_Q};
use crate::consensus_weights::{compute_final_weight_q, select_leader_deterministic, Weight};

/// Slot / consensus round identifier.
pub type Slot = u64;

/// Validator identifier (alias for NodeId – for readability).
pub type ValidatorId = NodeId;

/// Simple type for number of "raw" stake units.
/// This is what you store in state (e.g. number of TT-coins bonded).
pub type StakeRaw = u128;

/// Validator state in PRO consensus.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidatorState {
    /// PQ identifier (NodeId = Falcon key fingerprint).
    pub id: ValidatorId,

    /// Raw stake (e.g. number of bonded tokens).
    pub stake_raw: StakeRaw,

    /// Stake rescaled to [0,1] as Q32.32.
    pub stake_q: Q,

    /// Last quality from Golden Trio (Q32.32, [0,1]).
    pub quality_q: Q,

    /// Last computed RTT trust (T(v) ∈ [0,1] Q32.32).
    pub trust_q: TrustScore,
}

/// Main PRO consensus object.
pub struct ConsensusPro {
    /// RTT PRO – maintains trust(v) based on history and vouching.
    pub trust_graph: TrustGraph,

    /// Validators in map by NodeId.
    validators: HashMap<ValidatorId, ValidatorState>,

    /// Cache of total stake for normalization.
    total_stake_raw: StakeRaw,
}

impl ConsensusPro {
    /// Creates new instance with default RTT configuration.
    pub fn new_default() -> Self {
        let cfg = RTTConfig::default();
        Self::new(cfg)
    }

    pub fn new(rtt_cfg: RTTConfig) -> Self {
        Self {
            trust_graph: TrustGraph::new(rtt_cfg),
            validators: HashMap::new(),
            total_stake_raw: 0,
        }
    }

    /// Registers new validator with initial stake.
    ///
    /// Note: we don't check any economic rules here – that's for "staking" layer.
    pub fn register_validator(&mut self, id: ValidatorId, stake_raw: StakeRaw) {
        if self.validators.contains_key(&id) {
            // re-register → only update stake_raw, rest stays
            let v = self.validators.get_mut(&id).unwrap();
            // update total_stake_raw
            self.total_stake_raw = self
                .total_stake_raw
                .saturating_sub(v.stake_raw)
                .saturating_add(stake_raw);
            v.stake_raw = stake_raw;
            // stake_q will be recalculated in `recompute_all_stake_q`
            return;
        }

        self.total_stake_raw = self.total_stake_raw.saturating_add(stake_raw);

        // stake_q computed at normalization, quality/trust start at 0
        let state = ValidatorState {
            id,
            stake_raw,
            stake_q: 0,
            quality_q: 0,
            trust_q: 0,
        };

        self.validators.insert(id, state);
    }

    /// Removes validator (e.g. after unbonding / slashing).
    pub fn remove_validator(&mut self, id: &ValidatorId) {
        if let Some(v) = self.validators.remove(id) {
            self.total_stake_raw = self.total_stake_raw.saturating_sub(v.stake_raw);
        }
    }

    /// Updates raw stake of validator.
    pub fn update_stake_raw(&mut self, id: &ValidatorId, new_stake_raw: StakeRaw) {
        if let Some(v) = self.validators.get_mut(id) {
            self.total_stake_raw = self
                .total_stake_raw
                .saturating_sub(v.stake_raw)
                .saturating_add(new_stake_raw);
            v.stake_raw = new_stake_raw;
        }
    }

    /// Recomputes stake_q for all validators based on total_stake_raw.
    ///
    /// stake_q = min( stake_raw / total_stake_raw , 1.0 ) in Q32.32.
    pub fn recompute_all_stake_q(&mut self) {
        let total = self.total_stake_raw;
        if total == 0 {
            // nobody has stake – all 0
            for v in self.validators.values_mut() {
                v.stake_q = 0;
            }
            return;
        }

        for v in self.validators.values_mut() {
            let num = v.stake_raw.checked_shl(32).unwrap_or(u128::MAX); // * 2^32
            let stake_q = (num / total).min(u64::MAX as u128) as u64;
            v.stake_q = stake_q;
        }
    }

    /// Records validator quality (Golden Trio) in given epoch/slot
    /// and updates internal EWMA in RTT (history).
    ///
    /// `quality_q` ∈ [0, ONE_Q].
    pub fn record_quality(&mut self, id: &ValidatorId, quality_q: Q) {
        if let Some(v) = self.validators.get_mut(id) {
            v.quality_q = quality_q;
            self.trust_graph.record_quality(*id, quality_q);
        }
    }

    /// Helper version with f64 (only for analytical code / tests).
    pub fn record_quality_f64(&mut self, id: &ValidatorId, quality_f64: f64) {
        let q = q_from_f64(quality_f64);
        self.record_quality(id, q);
    }

    /// Computes RTT trust for single validator, updates state.
    pub fn update_validator_trust(&mut self, id: &ValidatorId) -> Option<TrustScore> {
        if !self.validators.contains_key(id) {
            return None;
        }

        let t = self.trust_graph.update_trust(*id);
        if let Some(v) = self.validators.get_mut(id) {
            v.trust_q = t;
        }
        Some(t)
    }

    /// Updates trust for entire list of validators (e.g. at end of epoch).
    pub fn update_all_trust(&mut self) {
        let ids: Vec<_> = self.validators.keys().cloned().collect();
        self.trust_graph.update_all(&ids);

        for id in ids {
            if let Some(v) = self.validators.get_mut(&id) {
                v.trust_q = self.trust_graph.get_trust(&id);
            }
        }
    }

    /// Returns current validator state.
    pub fn get_validator(&self, id: &ValidatorId) -> Option<&ValidatorState> {
        self.validators.get(id)
    }

    /// Current validator weight in consensus (per deterministic integer function).
    pub fn compute_validator_weight(&self, id: &ValidatorId) -> Option<Weight> {
        let v = self.validators.get(id)?;
        Some(compute_final_weight_q(v.trust_q, v.quality_q, v.stake_q))
    }

    /// Returns validator ranking by trust_q (only for debug / UI).
    pub fn get_trust_ranking(&self) -> Vec<(ValidatorId, TrustScore)> {
        let mut out: Vec<_> = self
            .validators
            .values()
            .map(|v| (v.id, v.trust_q))
            .collect();
        out.sort_by(|a, b| b.1.cmp(&a.1));
        out
    }

    /// Returns weight ranking (Weight) – useful e.g. for visualizing validator strength.
    pub fn get_weight_ranking(&self) -> Vec<(ValidatorId, Weight)> {
        let mut out: Vec<_> = self
            .validators
            .values()
            .map(|v| {
                let w = compute_final_weight_q(v.trust_q, v.quality_q, v.stake_q);
                (v.id, w)
            })
            .collect();
        out.sort_by(|a, b| b.1.cmp(&a.1));
        out
    }

    /// Leader selection for given beacon (RandomX/VRF) – deterministic.
    ///
    /// Note: beacon is a 32-byte hash from randomness layer.
    pub fn select_leader(&self, beacon: [u8; 32]) -> Option<ValidatorId> {
        let vals: Vec<_> = self
            .validators
            .values()
            .map(|v| (v.id, v.trust_q, v.quality_q, v.stake_q))
            .collect();
        select_leader_deterministic(beacon, &vals)
    }

    /// Debug: returns trust / quality / stake as f64 (ONLY for UI / logs).
    pub fn dump_debug_view(&self) -> Vec<(ValidatorId, f64, f64, f64)> {
        let mut out = Vec::new();
        for v in self.validators.values() {
            out.push((
                v.id,
                q_to_f64(v.trust_q),
                q_to_f64(v.quality_q),
                q_to_f64(v.stake_q),
            ));
        }
        out
    }
    /// Iterator over validators
    pub fn validators_iter(&self) -> impl Iterator<Item = (&ValidatorId, &ValidatorState)> {
        self.validators.iter()
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_id(byte: u8) -> NodeId {
        [byte; 32]
    }

    #[test]
    fn register_and_stake_normalization() {
        let mut c = ConsensusPro::new_default();

        let a = mk_id(1);
        let b = mk_id(2);

        c.register_validator(a, 100);
        c.register_validator(b, 300);

        c.recompute_all_stake_q();

        let va = c.get_validator(&a).unwrap();
        let vb = c.get_validator(&b).unwrap();

        let sa = q_to_f64(va.stake_q);
        let sb = q_to_f64(vb.stake_q);

        // 100 / 400 = 0.25, 300 / 400 = 0.75
        assert!((sa - 0.25).abs() < 1e-6, "sa={}", sa);
        assert!((sb - 0.75).abs() < 1e-6, "sb={}", sb);
    }

    #[test]
    fn trust_and_weight_ranking_basic() {
        let mut c = ConsensusPro::new_default();

        let a = mk_id(1);
        let b = mk_id(2);

        c.register_validator(a, 100);
        c.register_validator(b, 100);

        c.recompute_all_stake_q();

        // Jakości: A=0.9, B=0.6
        c.record_quality_f64(&a, 0.9);
        c.record_quality_f64(&b, 0.6);

        // Aktualizacja trustu
        c.update_all_trust();

        let ranking = c.get_trust_ranking();
        assert_eq!(ranking.len(), 2);
        // A powinien mieć >= trust niż B
        assert_eq!(ranking[0].0, a);

        let weight_ranking = c.get_weight_ranking();
        assert_eq!(weight_ranking.len(), 2);
        assert_eq!(weight_ranking[0].0, a);
    }

    #[test]
    fn leader_selection_is_deterministic_and_prefers_better_validator() {
        let mut c = ConsensusPro::new_default();

        let a = mk_id(1);
        let b = mk_id(2);

        c.register_validator(a, 1000);
        c.register_validator(b, 1000);

        c.recompute_all_stake_q();

        // A ma wyższy quality (i historycznie też go pompujemy)
        for _ in 0..10 {
            c.record_quality_f64(&a, 0.9);
            c.record_quality_f64(&b, 0.4);
        }
        c.update_all_trust();

        let beacon = [0x42u8; 32];
        let leader1 = c.select_leader(beacon).unwrap();
        let leader2 = c.select_leader(beacon).unwrap();
        assert_eq!(leader1, leader2);
        assert_eq!(leader1, a);
    }

    #[test]
    fn zero_total_stake_results_in_zero_stake_q() {
        let mut c = ConsensusPro::new_default();
        let a = mk_id(1);
        c.register_validator(a, 0);
        c.recompute_all_stake_q();
        let v = c.get_validator(&a).unwrap();
        assert_eq!(v.stake_q, 0);
    }

    #[test]
    fn everyone_full_stake_results_in_approx_one() {
        let mut c = ConsensusPro::new_default();
        let a = mk_id(1);
        let b = mk_id(2);

        c.register_validator(a, 100);
        c.register_validator(b, 100);

        c.recompute_all_stake_q();

        let va = c.get_validator(&a).unwrap();
        let vb = c.get_validator(&b).unwrap();

        // 100 / 200 = 0.5 dla obu
        assert_eq!(va.stake_q, vb.stake_q);
        assert!(va.stake_q <= ONE_Q);
    }
}