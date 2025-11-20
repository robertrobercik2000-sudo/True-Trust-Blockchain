#![forbid(unsafe_code)]

//! RTT (Recursive Trust Tree) - PRO VERSION (deterministic, consensus-grade)
//!
//! Changes from "research" version:
//! - Zero `f64` in core algorithm (no `exp`, `sigmoid`).
//! - All weights and results in Q32.32 (`u64`): deterministic on all CPUs.
//! - History as smoothed average (EWMA), no huge (validator, epoch) map.
//! - Vouching capped to [0,1] – doesn't "explode" with many edges.
//! - Simple, smooth S-curve: S(x) = 3x² − 2x³ instead of continuous sigmoid.
//!
//! Model:
//!   H(v) – historical quality (EWMA from Q(t))
//!   W(v) – quality from last epoch ("Golden Trio")
//!   V(v) – vouching (web of trust, normalized to [0,1])
//!
//!   Z_lin(v) = β₁·H(v) + β₂·V(v) + β₃·W(v) ∈ [0,1]
//!   T(v)     = S(Z_lin(v)) = 3z² − 2z³ ∈ [0,1]
//!
//! Everything in Q32.32, so suitable for consensus (fork-choice, validator selection).

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::node_id::NodeId;

/// Q32.32 fixed-point
pub type Q = u64;
pub const ONE_Q: Q = 1u64 << 32;

/// Trust score ∈ [0, ONE_Q]
pub type TrustScore = Q;

/// Quality score ∈ [0, ONE_Q]
pub type QualityScore = Q;

/// Epoch number
pub type Epoch = u64;

/// Convert f64 → Q (ONLY for helpers, e.g. defaults / tests)
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

/// Convert Q → f64 (e.g. for debug / visualization)
#[inline]
pub fn q_to_f64(x: Q) -> f64 {
    (x as f64) / (ONE_Q as f64)
}

/// Multiply Q32.32 · Q32.32 → Q32.32 (with truncation)
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

/// RTT Configuration (PRO version)
#[derive(Clone, Debug)]
pub struct RTTConfig {
    /// History weight (β₁) ∈ [0,1]
    pub beta_history: Q,

    /// Vouching weight (β₂) ∈ [0,1]
    pub beta_vouching: Q,

    /// Current work weight (β₃) ∈ [0,1]
    pub beta_work: Q,

    /// History memory coefficient (α) ∈ [0,1]
    /// H_new = α·H_old + (1-α)·Q_t
    pub alpha_history: Q,

    /// Minimum trust to vouch
    pub min_trust_to_vouch: Q,
}

impl Default for RTTConfig {
    fn default() -> Self {
        Self {
            // 0.4, 0.3, 0.3
            beta_history:  q_from_f64(0.4),
            beta_vouching: q_from_f64(0.3),
            beta_work:     q_from_f64(0.3),

            // α ≈ 0.99 → history forgets very slowly
            alpha_history: q_from_f64(0.99),

            // min trust to vouch: 0.5
            min_trust_to_vouch: q_from_f64(0.5),
        }
    }
}

impl RTTConfig {
    /// Verify: β₁ + β₂ + β₃ ≈ 1.0 (±1%)
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

/// Trust graph (PRO version, deterministic)
pub struct TrustGraph {
    /// Current trust score T(v) ∈ [0, ONE_Q]
    trust: HashMap<NodeId, TrustScore>,

    /// Smoothed history H(v) ∈ [0, ONE_Q] (EWMA from quality)
    history_h: HashMap<NodeId, Q>,

    /// Current quality (last epoch) W(v) ∈ [0, ONE_Q]
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

    /// Trust as f64 (ONLY for debug / UI)
    pub fn get_trust_f64(&self, validator: &NodeId) -> f64 {
        q_to_f64(self.get_trust(validator))
    }

    /// Set trust (internal use only)
    fn set_trust(&mut self, validator: NodeId, trust: TrustScore) {
        self.trust.insert(validator, qclamp01(trust));
    }

    /// Record quality score for given epoch
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
    /// Rules:
    /// - voucher must have trust ≥ min_trust_to_vouch
    /// - strength ≤ trust(voucher)
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

        // Cap to 1.0
        qclamp01(sum)
    }

    /// Compute work trust component W(v) ∈ [0, ONE_Q]
    ///
    /// Quality from last epoch (Golden Trio)
    pub fn compute_work_trust(&self, validator: &NodeId) -> Q {
        *self.last_quality.get(validator).unwrap_or(&0)
    }

    /// S-curve: S(x) = 3x² − 2x³ for x ∈ [0,1] (Q32.32)
    ///
    /// - monotonic increasing, smooth,
    /// - T(0)=0, T(1)=1,
    /// - gives "soft" saturation at top (softer than linear).
    fn q_scurve(x: Q) -> Q {
        let x = qclamp01(x);
        let x2 = qmul(x, x);     // x²
        let x3 = qmul(x2, x);    // x³

        let three_x2 = x2.saturating_mul(3);
        let two_x3 = x3.saturating_mul(2);

        three_x2.saturating_sub(two_x3).min(ONE_Q)
    }

    /// Update trust for validator (main algorithm, PRO version)
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

    /// Update all validators' trust (e.g. at end of epoch)
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

/// Bootstrap new validator with vouching.
///
/// `vouchers`: list of (voucher, strength) in Q.
/// Returns initial trust of new validator.
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
}