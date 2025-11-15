#![forbid(unsafe_code)]

//! RTT (Recursive Trust Tree) - FILAR I WAGA WAG!
//!
//! **Unikatowy algorytm trust:**
//! - Trust NIE jest liczbą - jest GRAFEM!
//! - Trzy komponenty: History + Vouching + Work
//! - Nonlinear (sigmoid function)
//! - Anti-Sybil (need vouching to bootstrap)
//!
//! **Formula:**
//! ```
//! Trust(v, t) = σ(β₁·H(t) + β₂·V(v) + β₃·W(v))
//!
//! gdzie:
//!   H(t) = Σ_{i=0}^{1000} e^(-λ·i) · Q(t-i)  (historical)
//!   V(v) = Σ_{j ∈ Peers} T(j) · vouch(j→v)   (vouching)
//!   W(v) = Golden Trio work score             (6 components)
//!   σ(x) = 1 / (1 + e^(-x))                   (sigmoid)
//! ```
//!
//! **Właściwości:**
//! - Historia ma znaczenie (exponential decay)
//! - Peer relations (web of trust)
//! - Emergent properties (sigmoid nonlinearity)
//! - PIERWSZY blockchain z RTT!

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Node ID (validator identifier)
pub type NodeId = [u8; 32];

/// Trust score ∈ [0.0, 1.0]
pub type TrustScore = f64;

/// Epoch number
pub type Epoch = u64;

/// RTT Configuration
#[derive(Clone, Debug)]
pub struct RTTConfig {
    /// Historical weight (β₁)
    pub beta_history: f64,
    
    /// Vouching weight (β₂)
    pub beta_vouching: f64,
    
    /// Work weight (β₃)
    pub beta_work: f64,
    
    /// Decay rate for history (λ)
    pub lambda_decay: f64,
    
    /// Lookback window (number of epochs)
    pub lookback_epochs: usize,
    
    /// Minimum trust to vouch
    pub min_trust_to_vouch: f64,
}

impl RTTConfig {
    /// Default configuration
    pub fn default() -> Self {
        Self {
            beta_history: 0.4,
            beta_vouching: 0.3,
            beta_work: 0.3,
            lambda_decay: 0.01,
            lookback_epochs: 1000,
            min_trust_to_vouch: 0.5,
        }
    }
    
    /// Verify weights sum to ~1.0
    pub fn verify(&self) -> bool {
        let sum = self.beta_history + self.beta_vouching + self.beta_work;
        (sum - 1.0).abs() < 0.01 // Allow 1% tolerance
    }
}

/// Quality score for single epoch ∈ [0.0, 1.0]
///
/// From Golden Trio 6 components:
/// - T1: Block production (30%)
/// - T2: Proof generation (25%)
/// - T3: Uptime (20%)
/// - T4: Stake lock (10%)
/// - T5: Fees (10%)
/// - T6: Network (5%)
pub type QualityScore = f64;

/// Vouching edge (who → whom, strength)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Vouch {
    /// Who vouches
    pub voucher: NodeId,
    
    /// For whom
    pub vouchee: NodeId,
    
    /// Strength ∈ [0.0, 1.0]
    pub strength: f64,
    
    /// Epoch when created
    pub created_at: Epoch,
}

/// Trust graph (all validators + edges)
pub struct TrustGraph {
    /// Current trust scores
    trust: HashMap<NodeId, TrustScore>,
    
    /// Historical quality scores (per validator, per epoch)
    /// Key: (validator, epoch) → QualityScore
    history: HashMap<(NodeId, Epoch), QualityScore>,
    
    /// Vouching edges
    /// Key: (voucher, vouchee) → Vouch
    vouches: HashMap<(NodeId, NodeId), Vouch>,
    
    /// Config
    config: RTTConfig,
}

impl TrustGraph {
    /// Create new empty graph
    pub fn new(config: RTTConfig) -> Self {
        assert!(config.verify(), "RTT config weights don't sum to 1.0!");
        
        Self {
            trust: HashMap::new(),
            history: HashMap::new(),
            vouches: HashMap::new(),
            config,
        }
    }
    
    /// Get current trust for validator
    pub fn get_trust(&self, validator: &NodeId) -> TrustScore {
        *self.trust.get(validator).unwrap_or(&0.0)
    }
    
    /// Set trust (internal use only)
    fn set_trust(&mut self, validator: NodeId, trust: TrustScore) {
        self.trust.insert(validator, trust.clamp(0.0, 1.0));
    }
    
    /// Record quality score for epoch
    pub fn record_quality(&mut self, validator: NodeId, epoch: Epoch, quality: QualityScore) {
        self.history.insert((validator, epoch), quality.clamp(0.0, 1.0));
    }
    
    /// Add vouching edge
    ///
    /// Returns true if successful, false if voucher doesn't have enough trust
    pub fn add_vouch(&mut self, vouch: Vouch) -> bool {
        // Check voucher has minimum trust
        let voucher_trust = self.get_trust(&vouch.voucher);
        if voucher_trust < self.config.min_trust_to_vouch {
            return false;
        }
        
        // Check strength <= voucher's trust
        if vouch.strength > voucher_trust {
            return false;
        }
        
        // Add edge
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
    
    /// Compute historical trust component (H)
    ///
    /// H(t) = Σ_{i=0}^{N} e^(-λ·i) · Q(t-i)
    ///
    /// Returns: weighted sum of quality scores (not normalized!)
    pub fn compute_historical_trust(&self, validator: &NodeId, current_epoch: Epoch) -> f64 {
        let lambda = self.config.lambda_decay;
        let lookback = self.config.lookback_epochs;
        
        let mut sum = 0.0;
        
        for i in 0..lookback {
            let past_epoch = current_epoch.saturating_sub(i as u64);
            
            if let Some(&quality) = self.history.get(&(*validator, past_epoch)) {
                let weight = (-lambda * (i as f64)).exp();
                sum += weight * quality;
            }
        }
        
        sum
    }
    
    /// Compute vouching trust component (V)
    ///
    /// V(v) = Σ_{j ∈ Peers} T(j) · vouch_strength(j→v)
    ///
    /// Returns: weighted sum (not normalized!)
    pub fn compute_vouching_trust(&self, validator: &NodeId) -> f64 {
        let mut sum = 0.0;
        
        for vouch in self.incoming_vouches(validator) {
            let voucher_trust = self.get_trust(&vouch.voucher);
            sum += voucher_trust * vouch.strength;
        }
        
        sum
    }
    
    /// Compute work trust component (W)
    ///
    /// W(v) = quality score z Golden Trio (ostatni epoch)
    ///
    /// Returns: quality ∈ [0.0, 1.0]
    pub fn compute_work_trust(&self, validator: &NodeId, current_epoch: Epoch) -> f64 {
        self.history
            .get(&(*validator, current_epoch))
            .copied()
            .unwrap_or(0.0)
    }
    
    /// Update trust for validator (main algorithm!)
    ///
    /// T(v, t) = σ(β₁·H(t) + β₂·V(v) + β₃·W(v))
    ///
    /// Returns: new trust score
    pub fn update_trust(&mut self, validator: NodeId, current_epoch: Epoch) -> TrustScore {
        // 1. Compute three components
        let h = self.compute_historical_trust(&validator, current_epoch);
        let v = self.compute_vouching_trust(&validator);
        let w = self.compute_work_trust(&validator, current_epoch);
        
        // 2. Weighted combination
        let z = self.config.beta_history * h +
                self.config.beta_vouching * v +
                self.config.beta_work * w;
        
        // 3. Sigmoid to bound [0, 1]
        let trust = sigmoid(z);
        
        // 4. Store
        self.set_trust(validator, trust);
        
        trust
    }
    
    /// Update all validators' trust (epoch transition)
    pub fn update_all(&mut self, validators: &[NodeId], current_epoch: Epoch) {
        for validator in validators {
            self.update_trust(*validator, current_epoch);
        }
    }
    
    /// Get trust ranking (sorted descending)
    pub fn get_ranking(&self) -> Vec<(NodeId, TrustScore)> {
        let mut ranking: Vec<_> = self.trust.iter()
            .map(|(id, &trust)| (*id, trust))
            .collect();
        
        ranking.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        
        ranking
    }
    
    /// Export graph for visualization
    pub fn export_dot(&self) -> String {
        let mut dot = String::from("digraph TrustGraph {\n");
        
        // Nodes
        for (id, &trust) in &self.trust {
            let label = format!("{:.2}", trust);
            let color = if trust > 0.8 { "green" } else if trust > 0.5 { "yellow" } else { "red" };
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
            dot.push_str(&format!(
                "  \"{}\" -> \"{}\" [label=\"{:.2}\"];\n",
                voucher_hex,
                vouchee_hex,
                vouch.strength
            ));
        }
        
        dot.push_str("}\n");
        dot
    }
}

/// Sigmoid function: σ(x) = 1 / (1 + e^(-x))
///
/// Maps (-∞, +∞) → (0, 1)
///
/// Examples:
/// - x=-5 → 0.007
/// - x=-2 → 0.119
/// - x=0  → 0.500
/// - x=2  → 0.881
/// - x=5  → 0.993
pub fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

/// Bootstrap new validator with vouching
///
/// New validators start with 0 trust but can bootstrap via vouching.
/// If 3 validators with trust [0.7, 0.85, 0.6] vouch with strength [0.6, 0.8, 0.5]:
///   V = 0.7×0.6 + 0.85×0.8 + 0.6×0.5 = 1.40
///   Trust = σ(0.3×1.40) = σ(0.42) ≈ 0.60
///
/// So new validator starts with 60% trust!
pub fn bootstrap_validator(
    graph: &mut TrustGraph,
    new_validator: NodeId,
    vouchers: Vec<(NodeId, f64)>, // (voucher, strength)
    epoch: Epoch,
) -> TrustScore {
    // Add vouches
    for (voucher, strength) in vouchers {
        let vouch = Vouch {
            voucher,
            vouchee: new_validator,
            strength,
            created_at: epoch,
        };
        
        if !graph.add_vouch(vouch) {
            eprintln!("⚠️ Vouch from {:?} rejected (insufficient trust)", &voucher[0..4]);
        }
    }
    
    // Update trust (H=0, W=0, tylko V counts)
    graph.update_trust(new_validator, epoch)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sigmoid() {
        assert!((sigmoid(-5.0) - 0.007).abs() < 0.001);
        assert!((sigmoid(-2.0) - 0.119).abs() < 0.001);
        assert!((sigmoid(0.0) - 0.500).abs() < 0.001);
        assert!((sigmoid(2.0) - 0.881).abs() < 0.001);
        assert!((sigmoid(5.0) - 0.993).abs() < 0.001);
    }
    
    #[test]
    fn test_trust_graph_basic() {
        let config = RTTConfig::default();
        let mut graph = TrustGraph::new(config);
        
        let alice = [1u8; 32];
        let bob = [2u8; 32];
        
        // Set initial trust manually (for testing)
        graph.set_trust(alice, 0.8);
        graph.set_trust(bob, 0.6);
        
        assert_eq!(graph.get_trust(&alice), 0.8);
        assert_eq!(graph.get_trust(&bob), 0.6);
    }
    
    #[test]
    fn test_historical_trust() {
        let config = RTTConfig::default();
        let mut graph = TrustGraph::new(config);
        
        let alice = [1u8; 32];
        
        // Record quality scores over 10 epochs
        for epoch in 0..10 {
            graph.record_quality(alice, epoch, 0.9); // Consistently good
        }
        
        let h = graph.compute_historical_trust(&alice, 9);
        
        // Should be high (many good epochs with decay)
        assert!(h > 8.0, "Historical trust: {}", h);
    }
    
    #[test]
    fn test_vouching() {
        let config = RTTConfig::default();
        let mut graph = TrustGraph::new(config);
        
        let alice = [1u8; 32];
        let bob = [2u8; 32];
        let carol = [3u8; 32];
        
        // Set trust for vouchers
        graph.set_trust(alice, 0.9);
        graph.set_trust(bob, 0.7);
        
        // Alice vouches for Carol
        let vouch1 = Vouch {
            voucher: alice,
            vouchee: carol,
            strength: 0.8,
            created_at: 0,
        };
        assert!(graph.add_vouch(vouch1));
        
        // Bob vouches for Carol
        let vouch2 = Vouch {
            voucher: bob,
            vouchee: carol,
            strength: 0.6,
            created_at: 0,
        };
        assert!(graph.add_vouch(vouch2));
        
        // Compute vouching trust for Carol
        let v = graph.compute_vouching_trust(&carol);
        
        // V = 0.9×0.8 + 0.7×0.6 = 0.72 + 0.42 = 1.14
        assert!((v - 1.14).abs() < 0.01, "Vouching trust: {}", v);
    }
    
    #[test]
    fn test_full_trust_update() {
        let config = RTTConfig::default();
        let mut graph = TrustGraph::new(config);
        
        let alice = [1u8; 32];
        
        // History: record good quality over 10 epochs
        for epoch in 0..10 {
            graph.record_quality(alice, epoch, 0.9);
        }
        
        // Work: current epoch quality
        graph.record_quality(alice, 10, 0.95);
        
        // Vouching: none (no vouches yet)
        
        // Update trust
        let trust = graph.update_trust(alice, 10);
        
        // Should be high (good history + good work, no vouching)
        // H ≈ 8.5, V = 0, W = 0.95
        // z = 0.4×8.5 + 0.3×0 + 0.3×0.95 = 3.4 + 0.285 = 3.685
        // σ(3.685) ≈ 0.975
        assert!(trust > 0.97, "Trust: {}", trust);
    }
    
    #[test]
    fn test_bootstrap_validator() {
        let config = RTTConfig::default();
        let mut graph = TrustGraph::new(config);
        
        // Existing validators
        let alice = [1u8; 32];
        let bob = [2u8; 32];
        let carol = [3u8; 32];
        
        graph.set_trust(alice, 0.9);
        graph.set_trust(bob, 0.7);
        graph.set_trust(carol, 0.6);
        
        // New validator (Eve)
        let eve = [4u8; 32];
        
        // Bootstrap with vouches
        let vouchers = vec![
            (alice, 0.8),
            (bob, 0.6),
            (carol, 0.5),
        ];
        
        let trust = bootstrap_validator(&mut graph, eve, vouchers, 0);
        
        // V = 0.9×0.8 + 0.7×0.6 + 0.6×0.5 = 1.44
        // z = 0.3×1.44 = 0.432
        // σ(0.432) ≈ 0.606
        assert!((trust - 0.606).abs() < 0.01, "Bootstrapped trust: {}", trust);
    }
    
    #[test]
    fn test_trust_ranking() {
        let config = RTTConfig::default();
        let mut graph = TrustGraph::new(config);
        
        let alice = [1u8; 32];
        let bob = [2u8; 32];
        let carol = [3u8; 32];
        
        graph.set_trust(alice, 0.9);
        graph.set_trust(bob, 0.6);
        graph.set_trust(carol, 0.85);
        
        let ranking = graph.get_ranking();
        
        assert_eq!(ranking.len(), 3);
        assert_eq!(ranking[0].0, alice); // Highest
        assert_eq!(ranking[1].0, carol);
        assert_eq!(ranking[2].0, bob); // Lowest
    }
}
