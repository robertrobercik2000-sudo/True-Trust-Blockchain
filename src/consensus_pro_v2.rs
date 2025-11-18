#![forbid(unsafe_code)]

//! TRUE_TRUST – Consensus PRO (PQ-only, deterministyczny)
//!
//! Ten moduł spina:
//! - RTT PRO (`crate::rtt_pro`) – trust T(v) ∈ [0,1] (Q32.32),
//! - „Golden Trio" quality (Q(v) ∈ [0,1] Q32.32 – zasilane z warstwy wykonawczej),
//! - stake (S(v) – znormalizowany do [0,1] Q32.32),
//! - deterministyczne wagi (`crate::consensus_weights`).
//!
//! Zero `f64` w ścieżce konsensusu (leader selection, fork-choice). F64
//! można używać tylko w funkcjach *_debug, które nie są używane na hot-path.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::node_id::NodeId;
use crate::rtt_pro::{q_from_f64, q_to_f64, Q, TrustGraph, TrustScore, RTTConfig, ONE_Q};
use crate::consensus_weights::{compute_final_weight_q, select_leader_deterministic, Weight};

/// Identyfikator slotu / rundy konsensusu.
pub type Slot = u64;

/// Identyfikator walidatora (alias na NodeId – tutaj dla czytelności).
pub type ValidatorId = NodeId;

/// Prosty typ na liczbę „surowych" jednostek stake.
/// To jest to, co zapisujesz w stanie (np. liczba TT-coins w bondingu).
pub type StakeRaw = u128;

/// Stan walidatora w konsensusie PRO.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidatorState {
    /// PQ identyfikator (NodeId = fingerprint klucza Falcon).
    pub id: ValidatorId,

    /// Surowy stake (np. liczba tokenów w bondingu).
    pub stake_raw: StakeRaw,

    /// Stake przeskalowany do [0,1] jako Q32.32.
    pub stake_q: Q,

    /// Ostatnia jakość z Golden Trio (Q32.32, [0,1]).
    pub quality_q: Q,

    /// Ostatnio policzony trust RTT (T(v) ∈ [0,1] Q32.32).
    pub trust_q: TrustScore,
}

/// Główny obiekt konsensusu PRO.
pub struct ConsensusPro {
    /// RTT PRO – utrzymuje trust(v) na bazie historii i vouchingu.
    pub trust_graph: TrustGraph,

    /// Walidatorzy w mapie po NodeId.
    validators: HashMap<ValidatorId, ValidatorState>,

    /// Cache sumy stake dla normalizacji.
    total_stake_raw: StakeRaw,
}

impl ConsensusPro {
    /// Tworzy nową instancję z domyślną konfiguracją RTT.
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

    /// Rejestruje nowego walidatora z początkowym stake.
    ///
    /// Uwaga: nie sprawdzamy tu żadnych reguł ekonomicznych – to ma zrobić layer „staking".
    pub fn register_validator(&mut self, id: ValidatorId, stake_raw: StakeRaw) {
        if self.validators.contains_key(&id) {
            // re-register → tylko aktualizujemy stake_raw, reszta zostaje
            let v = self.validators.get_mut(&id).unwrap();
            // aktualizuj total_stake_raw
            self.total_stake_raw = self
                .total_stake_raw
                .saturating_sub(v.stake_raw)
                .saturating_add(stake_raw);
            v.stake_raw = stake_raw;
            // stake_q zostanie przeliczone w `recompute_all_stake_q`
            return;
        }

        self.total_stake_raw = self.total_stake_raw.saturating_add(stake_raw);

        // stake_q policzymy przy normalizacji, quality/trust startują z 0
        let state = ValidatorState {
            id,
            stake_raw,
            stake_q: 0,
            quality_q: 0,
            trust_q: 0,
        };

        self.validators.insert(id, state);
    }

    /// Usuwa walidatora (np. po unbondażu / karze).
    pub fn remove_validator(&mut self, id: &ValidatorId) {
        if let Some(v) = self.validators.remove(id) {
            self.total_stake_raw = self.total_stake_raw.saturating_sub(v.stake_raw);
        }
    }

    /// Aktualizuje surowy stake walidatora.
    pub fn update_stake_raw(&mut self, id: &ValidatorId, new_stake_raw: StakeRaw) {
        if let Some(v) = self.validators.get_mut(id) {
            self.total_stake_raw = self
                .total_stake_raw
                .saturating_sub(v.stake_raw)
                .saturating_add(new_stake_raw);
            v.stake_raw = new_stake_raw;
        }
    }

    /// Przelicza stake_q dla wszystkich walidatorów na bazie total_stake_raw.
    ///
    /// stake_q = min( stake_raw / total_stake_raw , 1.0 ) w Q32.32.
    pub fn recompute_all_stake_q(&mut self) {
        let total = self.total_stake_raw;
        if total == 0 {
            // nikt nie ma stake – wszyscy 0
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

    /// Zapisuje jakość walidatora (Golden Trio) w danej epoce/slotcie
    /// oraz update'uje wewnętrzny EWMA w RTT (history).
    ///
    /// `quality_q` ∈ [0, ONE_Q].
    pub fn record_quality(&mut self, id: &ValidatorId, quality_q: Q) {
        if let Some(v) = self.validators.get_mut(id) {
            v.quality_q = quality_q;
            self.trust_graph.record_quality(*id, quality_q);
        }
    }

    /// Wersja pomocnicza z f64 (tylko dla kodu analitycznego / tests).
    pub fn record_quality_f64(&mut self, id: &ValidatorId, quality_f64: f64) {
        let q = q_from_f64(quality_f64);
        self.record_quality(id, q);
    }

    /// Liczy trust RTT dla pojedynczego walidatora, aktualizuje stan.
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

    /// Aktualizuje trust dla całej listy walidatorów (np. na koniec epoki).
    pub fn update_all_trust(&mut self) {
        let ids: Vec<_> = self.validators.keys().cloned().collect();
        self.trust_graph.update_all(&ids);

        for id in ids {
            if let Some(v) = self.validators.get_mut(&id) {
                v.trust_q = self.trust_graph.get_trust(&id);
            }
        }
    }

    /// Zwraca bieżący stan walidatora.
    pub fn get_validator(&self, id: &ValidatorId) -> Option<&ValidatorState> {
        self.validators.get(id)
    }

    /// Bieżąca waga walidatora w konsensusie (wg deterministycznej funkcji integerowej).
    pub fn compute_validator_weight(&self, id: &ValidatorId) -> Option<Weight> {
        let v = self.validators.get(id)?;
        Some(compute_final_weight_q(v.trust_q, v.quality_q, v.stake_q))
    }

    /// Zwraca ranking walidatorów według trust_q (tylko do debug / UI).
    pub fn get_trust_ranking(&self) -> Vec<(ValidatorId, TrustScore)> {
        let mut out: Vec<_> = self
            .validators
            .values()
            .map(|v| (v.id, v.trust_q))
            .collect();
        out.sort_by(|a, b| b.1.cmp(&a.1));
        out
    }

    /// Zwraca ranking wag (Weight) – przydatne np. do wizualizacji siły walidatorów.
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

    /// Wybór lidera dla danego beacona (RandomX/VRF) – deterministyczny.
    ///
    /// Uwaga: beacon to 32-bajtowy hash z warstwy losowości.
    pub fn select_leader(&self, beacon: [u8; 32]) -> Option<ValidatorId> {
        let vals: Vec<_> = self
            .validators
            .values()
            .map(|v| (v.id, v.trust_q, v.quality_q, v.stake_q))
            .collect();
        select_leader_deterministic(beacon, &vals)
    }

    /// Debug: zwraca trust / quality / stake jako f64 (TYLKO do UI / logów).
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
