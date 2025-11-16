#![forbid(unsafe_code)]

//! RTT (Recursive Trust Tree) - WERSJA PRO (deterministyczna, pod konsensus)
//!
//! Zmiany względem starej wersji:
//! - Zero `f64` w rdzeniu algorytmu (brak `exp`, `sigmoid`).
//! - Wszystkie wagi i wyniki w Q32.32 (`u64`): deterministyczne na wszystkich CPU.
//! - Historia jako wygładzona średnia (EWMA), bez gigantycznej mapy (validator, epoch).
//! - Vouching ograniczony do [0,1] – nie „wybucha" przy wielu krawędziach.
//! - Prosta, gładka krzywa S: S(x) = 3x² − 2x³ zamiast ciągłego sigmoidu.
//!
//! Model:
//!   H(v) – historyczna jakość (EWMA z Q(t))
//!   W(v) – jakość z ostatniej epoki („Golden Trio")
//!   V(v) – vouching (web of trust, znormalizowany do [0,1])
//!
//!   Z_lin(v) = β₁·H(v) + β₂·V(v) + β₃·W(v) ∈ [0,1]
//!   T(v)     = S(Z_lin(v)) = 3z² − 2z³ ∈ [0,1]
//!
//! Wszystko w Q32.32, więc nadaje się do użycia w konsensusie (fork-choice, wybór validatorów).

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Q32.32 fixed-point
pub type Q = u64;
pub const ONE_Q: Q = 1u64 << 32;

/// Node ID (validator identifier, np. Falcon public key)
pub type NodeId = [u8; 32];

/// Trust score ∈ [0, ONE_Q]
pub type TrustScore = Q;

/// Quality score ∈ [0, ONE_Q]
pub type QualityScore = Q;

/// Epoch number
pub type Epoch = u64;

/// Konwersja z f64 → Q (TYLKO pomocniczo, np. do defaultów / testów)
#[inline]
pub fn q_from_f64(x: f64) -> Q {
    if x <= 0.0 {
        return 0;
    }
    if x >= 1.0 {
        return ONE_Q;
    }
    (x * (ONE_Q as f64)) as u64
}

/// Konwersja z Q → f64 (np. do debug / wizualizacji)
#[inline]
pub fn q_to_f64(x: Q) -> f64 {
    (x as f64) / (ONE_Q as f64)
}

/// Mnożenie Q32.32 · Q32.32 → Q32.32 (z obcięciem)
#[inline]
pub fn qmul(a: Q, b: Q) -> Q {
    let z = (a as u128) * (b as u128);
    let shifted = z >> 32;
    shifted.min(u64::MAX as u128) as u64
}

/// Clamp do [0, 1]
#[inline]
pub fn qclamp01(x: Q) -> Q {
    x.min(ONE_Q)
}

/// RTT Configuration (wersja PRO)
#[derive(Clone, Debug)]
pub struct RTTConfig {
    /// Waga historii (β₁) ∈ [0,1]
    pub beta_history: Q,

    /// Waga vouchingu (β₂) ∈ [0,1]
    pub beta_vouching: Q,

    /// Waga bieżącej pracy (β₃) ∈ [0,1]
    pub beta_work: Q,

    /// Współczynnik pamięci historii (α) ∈ [0,1]
    /// H_new = α·H_old + (1-α)·Q_t
    pub alpha_history: Q,

    /// Minimalny trust do vouchowania
    pub min_trust_to_vouch: Q,
}

impl Default for RTTConfig {
    fn default() -> Self {
        Self {
            // 0.4, 0.3, 0.3
            beta_history:  q_from_f64(0.4),
            beta_vouching: q_from_f64(0.3),
            beta_work:     q_from_f64(0.3),

            // α ≈ 0.99 → historia zapomina bardzo wolno
            alpha_history: q_from_f64(0.99),

            // min trust do vouchowania: 0.5
            min_trust_to_vouch: q_from_f64(0.5),
        }
    }
}

impl RTTConfig {
    /// Weryfikacja: β₁ + β₂ + β₃ ≈ 1.0 (±1%)
    pub fn verify(&self) -> bool {
        let sum = self.beta_history
            .saturating_add(self.beta_vouching)
            .saturating_add(self.beta_work);
        let diff = if sum > ONE_Q { sum - ONE_Q } else { ONE_Q - sum };
        diff < q_from_f64(0.01)
    }
}

/// Vouching edge (who → whom, strength)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Vouch {
    /// Who vouches
    pub voucher: NodeId,

    /// For whom
    pub vouchee: NodeId,

    /// Strength ∈ [0, ONE_Q]
    pub strength: Q,

    /// Epoch when created
    pub created_at: Epoch,
}

/// Trust graph (wersja PRO, deterministyczna)
pub struct TrustGraph {
    /// Aktualne trust score T(v) ∈ [0, ONE_Q]
    trust: HashMap<NodeId, TrustScore>,

    /// Wygładzona historia H(v) ∈ [0, ONE_Q] (EWMA z jakości)
    history_h: HashMap<NodeId, Q>,

    /// Bieżąca jakość (ostatnia epoka) W(v) ∈ [0, ONE_Q]
    last_quality: HashMap<NodeId, QualityScore>,

    /// Vouching edges
    /// Key: (voucher, vouchee) → Vouch
    vouches: HashMap<(NodeId, NodeId), Vouch>,

    /// Config
    config: RTTConfig,
}

impl TrustGraph {
    /// Create new empty graph (PRO config)
    pub fn new(config: RTTConfig) -> Self {
        assert!(config.verify(), "RTT config weights don't sum to 1.0!");
        Self {
            trust: HashMap::new(),
            history_h: HashMap::new(),
            last_quality: HashMap::new(),
            vouches: HashMap::new(),
            config,
        }
    }

    /// Get current trust for validator (Q)
    pub fn get_trust(&self, validator: &NodeId) -> TrustScore {
        *self.trust.get(validator).unwrap_or(&0)
    }

    /// Trust jako f64 (TYLKO do debug / UI)
    pub fn get_trust_f64(&self, validator: &NodeId) -> f64 {
        q_to_f64(self.get_trust(validator))
    }

    /// Set trust (internal use only)
    fn set_trust(&mut self, validator: NodeId, trust: TrustScore) {
        self.trust.insert(validator, qclamp01(trust));
    }

    /// Zarejestrowanie quality score dla danej epoki
    ///
    /// quality ∈ [0, ONE_Q]
    pub fn record_quality(&mut self, validator: NodeId, quality: QualityScore) {
        let q = qclamp01(quality);
        self.last_quality.insert(validator, q);

        let prev_h = *self.history_h.get(&validator).unwrap_or(&0);
        let alpha = self.config.alpha_history;
        let one_minus_alpha = ONE_Q - alpha;

        // H_new = α·H_old + (1-α)·q
        let part_old = qmul(alpha, prev_h);
        let part_new = qmul(one_minus_alpha, q);
        let h_new = part_old.saturating_add(part_new);

        self.history_h.insert(validator, qclamp01(h_new));
    }

    /// Add vouching edge
    ///
    /// Zasady:
    /// - voucher musi mieć trust ≥ min_trust_to_vouch
    /// - strength ≤ trust(vouchera)
    pub fn add_vouch(&mut self, vouch: Vouch) -> bool {
        let voucher_trust = self.get_trust(&vouch.voucher);
        if voucher_trust < self.config.min_trust_to_vouch {
            return false;
        }
        if vouch.strength > voucher_trust {
            return false;
        }

        let key = (vouch.voucher, vouch.vouchee);
        self.vouches.insert(key, vouch);
        true
    }

    /// Remove vouching edge
    pub fn remove_vouch(&mut self, voucher: &NodeId, vouchee: &NodeId) {
        let key = (*voucher, *vouchee);
        self.vouches.remove(&key);
    }

    /// Get all incoming vouches for validator
    pub fn incoming_vouches(&self, validator: &NodeId) -> Vec<&Vouch> {
        self.vouches
            .values()
            .filter(|v| &v.vouchee == validator)
            .collect()
    }

    /// Compute historical trust component H(v) ∈ [0, ONE_Q]
    pub fn compute_historical_trust(&self, validator: &NodeId) -> Q {
        *self.history_h.get(validator).unwrap_or(&0)
    }

    /// Compute vouching trust component V(v) ∈ [0, ONE_Q]
    ///
    /// V(v) = min( Σ T(j)·strength(j→v), 1.0 )
    pub fn compute_vouching_trust(&self, validator: &NodeId) -> Q {
        let mut sum: Q = 0;

        for v in self.incoming_vouches(validator) {
            let voucher_trust = self.get_trust(&v.voucher);
            let contrib = qmul(voucher_trust, v.strength);
            sum = sum.saturating_add(contrib);
        }

        // Cap do 1.0
        qclamp01(sum)
    }

    /// Compute work trust component W(v) ∈ [0, ONE_Q]
    ///
    /// Jakość z ostatniej epoki (Golden Trio)
    pub fn compute_work_trust(&self, validator: &NodeId) -> Q {
        *self.last_quality.get(validator).unwrap_or(&0)
    }

    /// Krzywa S: S(x) = 3x² − 2x³ dla x ∈ [0,1] (Q32.32)
    ///
    /// - rosnąca, gładka,
    /// - T(0)=0, T(1)=1,
    /// - daje „łagodne" nasycanie na górze (bardziej miękko niż linia).
    fn q_scurve(x: Q) -> Q {
        let x = qclamp01(x);
        let x2 = qmul(x, x);     // x²
        let x3 = qmul(x2, x);    // x³

        let three_x2 = x2.saturating_mul(3);
        let two_x3 = x3.saturating_mul(2);

        three_x2.saturating_sub(two_x3).min(ONE_Q)
    }

    /// Update trust for validator (main algorithm, PRO wersja)
    ///
    /// Z_lin = β₁·H + β₂·V + β₃·W
    /// T     = S(Z_lin)
    pub fn update_trust(&mut self, validator: NodeId) -> TrustScore {
        let h = self.compute_historical_trust(&validator); // [0,1]
        let v = self.compute_vouching_trust(&validator);   // [0,1]
        let w = self.compute_work_trust(&validator);       // [0,1]

        let cfg = &self.config;

        let z_h = qmul(cfg.beta_history, h);
        let z_v = qmul(cfg.beta_vouching, v);
        let z_w = qmul(cfg.beta_work, w);

        let z_lin = z_h
            .saturating_add(z_v)
            .saturating_add(z_w)
            .min(ONE_Q);

        let trust = Self::q_scurve(z_lin);

        self.set_trust(validator, trust);
        trust
    }

    /// Update all validators' trust (np. na koniec epoki)
    pub fn update_all(&mut self, validators: &[NodeId]) {
        for validator in validators {
            self.update_trust(*validator);
        }
    }

    /// Get trust ranking (sorted descending, Q)
    pub fn get_ranking(&self) -> Vec<(NodeId, TrustScore)> {
        let mut ranking: Vec<_> = self.trust
            .iter()
            .map(|(id, &trust)| (*id, trust))
            .collect();

        ranking.sort_by(|a, b| b.1.cmp(&a.1));
        ranking
    }

    /// Export graph for visualization (DOT / Graphviz)
    pub fn export_dot(&self) -> String {
        let mut dot = String::from("digraph TrustGraph {\n");

        // Nodes
        for (id, &trust_q) in &self.trust {
            let trust_f = q_to_f64(trust_q);
            let label = format!("{:.2}", trust_f);
            let color = if trust_f > 0.8 {
                "green"
            } else if trust_f > 0.5 {
                "yellow"
            } else {
                "red"
            };
            dot.push_str(&format!(
                "  \"{}\" [label=\"{}\", color={}, style=filled];\n",
                hex::encode(&id[0..4]),
                label,
                color
            ));
        }

        // Edges
        for vouch in self.vouches.values() {
            let voucher_hex = hex::encode(&vouch.voucher[0..4]);
            let vouchee_hex = hex::encode(&vouch.vouchee[0..4]);
            let strength_f = q_to_f64(vouch.strength);
            dot.push_str(&format!(
                "  \"{}\" -> \"{}\" [label=\"{:.2}\"];\n",
                voucher_hex,
                vouchee_hex,
                strength_f
            ));
        }

        dot.push_str("}\n");
        dot
    }
}

/// Bootstrap nowego walidatora z vouchingiem.
///
/// `vouchers`: lista (voucher, strength) w Q.
/// Zwraca początkowy trust nowego walidatora.
pub fn bootstrap_validator(
    graph: &mut TrustGraph,
    new_validator: NodeId,
    vouchers: Vec<(NodeId, Q)>,
) -> TrustScore {
    for (voucher, strength) in vouchers {
        let vouch = Vouch {
            voucher,
            vouchee: new_validator,
            strength: qclamp01(strength),
            created_at: 0,
        };
        let _ = graph.add_vouch(vouch);
    }

    graph.update_trust(new_validator)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qmul_basic() {
        let half = q_from_f64(0.5);
        let quarter = qmul(half, half);
        assert!((q_to_f64(quarter) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn test_scurve_shape() {
        let zero = TrustGraph::q_scurve(0);
        let mid = TrustGraph::q_scurve(q_from_f64(0.5));
        let one = TrustGraph::q_scurve(ONE_Q);

        assert!(zero <= q_from_f64(0.01));
        assert!(mid > q_from_f64(0.5)); // powyżej 0.5
        assert!((q_to_f64(one) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_trust_graph_basic() {
        let config = RTTConfig::default();
        let mut graph = TrustGraph::new(config);

        let alice = [1u8; 32];
        let bob = [2u8; 32];

        graph.set_trust(alice, q_from_f64(0.8));
        graph.set_trust(bob, q_from_f64(0.6));

        assert!((q_to_f64(graph.get_trust(&alice)) - 0.8).abs() < 1e-6);
        assert!((q_to_f64(graph.get_trust(&bob)) - 0.6).abs() < 1e-6);
    }

    #[test]
    fn test_historical_ewma() {
        let config = RTTConfig::default();
        let mut graph = TrustGraph::new(config);

        let alice = [1u8; 32];

        // 10 epok z jakością 0.9
        for _epoch in 0..10 {
            graph.record_quality(alice, q_from_f64(0.9));
        }

        let h = graph.compute_historical_trust(&alice);
        let hf = q_to_f64(h);

        // H powinno być wysokie, ale < 1.0
        assert!(hf > 0.8 && hf < 1.0, "Historical EWMA: {}", hf);
    }

    #[test]
    fn test_vouching_component() {
        let config = RTTConfig::default();
        let mut graph = TrustGraph::new(config);

        let alice = [1u8; 32];
        let bob = [2u8; 32];
        let carol = [3u8; 32];

        graph.set_trust(alice, q_from_f64(0.9));
        graph.set_trust(bob, q_from_f64(0.7));
        graph.set_trust(carol, q_from_f64(0.6));

        // Alice vouchuje Carol
        let v1 = Vouch {
            voucher: alice,
            vouchee: carol,
            strength: q_from_f64(0.8),
            created_at: 0,
        };
        assert!(graph.add_vouch(v1));

        // Bob vouchuje Carol
        let v2 = Vouch {
            voucher: bob,
            vouchee: carol,
            strength: q_from_f64(0.6),
            created_at: 0,
        };
        assert!(graph.add_vouch(v2));

        let v_q = graph.compute_vouching_trust(&carol);
        let v_f = q_to_f64(v_q);

        // 0.9*0.8 + 0.7*0.6 = 0.72 + 0.42 = 1.14 → capped do 1.0
        assert!((v_f - 1.0).abs() < 1e-6, "Vouching trust: {}", v_f);
    }

    #[test]
    fn test_full_trust_update() {
        let config = RTTConfig::default();
        let mut graph = TrustGraph::new(config);

        let alice = [1u8; 32];

        // Historia: 10 epok po 0.9
        for _ in 0..10 {
            graph.record_quality(alice, q_from_f64(0.9));
        }

        // Bieżąca jakość 0.95
        graph.record_quality(alice, q_from_f64(0.95));

        // Brak vouchingu
        let t = graph.update_trust(alice);

        let tf = q_to_f64(t);
        // Powinno być wysoko, ale < 1.0 (S-curve)
        assert!(tf > 0.9 && tf < 1.0, "Trust: {}", tf);
    }

    #[test]
    fn test_bootstrap_validator() {
        let config = RTTConfig::default();
        let mut graph = TrustGraph::new(config);

        let alice = [1u8; 32];
        let bob = [2u8; 32];
        let carol = [3u8; 32];
        let eve = [4u8; 32];

        graph.set_trust(alice, q_from_f64(0.9));
        graph.set_trust(bob, q_from_f64(0.7));
        graph.set_trust(carol, q_from_f64(0.6));

        // Eve bootstrap z trzema vouchami
        let vouchers = vec![
            (alice, q_from_f64(0.8)),
            (bob, q_from_f64(0.6)),
            (carol, q_from_f64(0.5)),
        ];

        let t = bootstrap_validator(&mut graph, eve, vouchers);
        let tf = q_to_f64(t);

        // Oczekujemy „środka": coś około 0.5–0.8 w zależności od S-curve
        assert!(tf > 0.5 && tf < 0.9, "Bootstrapped trust: {}", tf);
    }

    #[test]
    fn test_trust_ranking() {
        let config = RTTConfig::default();
        let mut graph = TrustGraph::new(config);

        let alice = [1u8; 32];
        let bob = [2u8; 32];
        let carol = [3u8; 32];

        graph.set_trust(alice, q_from_f64(0.9));
        graph.set_trust(bob, q_from_f64(0.6));
        graph.set_trust(carol, q_from_f64(0.85));

        let ranking = graph.get_ranking();

        assert_eq!(ranking.len(), 3);
        assert_eq!(ranking[0].0, alice); // 0.9
        assert_eq!(ranking[1].0, carol); // 0.85
        assert_eq!(ranking[2].0, bob);   // 0.6
    }
}
