#![forbid(unsafe_code)]

//! consensus_pro.rs - Production Consensus Adapter
//!
//! Łączy RTT PRO (Q32.32) + RandomX FFI (Monero) + Golden Trio
//! w jedną spójną fasadę dla pot_node.rs i node.rs

use crate::rtt_trust_pro::{TrustGraph, RTTConfig, TrustScore, Q, q_from_f64, q_to_f64};
use crate::pot::NodeId;
use std::collections::HashMap;

// RandomXHasher from FFI (requires librandomx installed)
// If build fails, install RandomX: see RANDOMX_INSTALL.md
#[cfg(all(feature = "randomx-ffi", target_pointer_width = "64"))]
use crate::pow_randomx_monero::RandomXHasher;

// === STARK (Winterfell) PQ TX integration ===
use crate::pq::tx_stark::TransactionStark;
use crate::core::Hash32;

/// Consensus PRO - główna fasada
pub struct ConsensusPro {
    /// RTT Trust graph (Q32.32 deterministyczny)
    pub trust_graph: TrustGraph,
    
    /// RandomX hasher (Monero FFI)
    #[cfg(all(feature = "randomx-ffi", target_pointer_width = "64"))]
    randomx_hasher: Option<RandomXHasher>,
    
    /// Current epoch
    current_epoch: u64,
}

impl ConsensusPro {
    /// Nowy consensus z domyślnym RTT config
    pub fn new() -> Self {
        Self {
            trust_graph: TrustGraph::new(RTTConfig::default()),
            #[cfg(all(feature = "randomx-ffi", target_pointer_width = "64"))]
            randomx_hasher: None,
            current_epoch: 0,
        }
    }
    
    /// Nowy consensus z custom RTT config
    pub fn with_config(config: RTTConfig) -> Self {
        Self {
            trust_graph: TrustGraph::new(config),
            #[cfg(all(feature = "randomx-ffi", target_pointer_width = "64"))]
            randomx_hasher: None,
            current_epoch: 0,
        }
    }
    
    /// Inicjalizuj RandomX dla epoki
    #[cfg(all(feature = "randomx-ffi", target_pointer_width = "64"))]
    pub fn init_randomx(&mut self, epoch: u64) {
        self.current_epoch = epoch;
        self.randomx_hasher = Some(RandomXHasher::new(epoch));
    }
    
    #[cfg(not(all(feature = "randomx-ffi", target_pointer_width = "64")))]
    pub fn init_randomx(&mut self, epoch: u64) {
        self.current_epoch = epoch;
    }
    
    /// RandomX hash (używa Monero FFI)
    pub fn randomx_hash(&self, input: &[u8]) -> [u8; 32] {
        #[cfg(all(feature = "randomx-ffi", target_pointer_width = "64"))]
        {
            if let Some(ref hasher) = self.randomx_hasher {
                return hasher.hash(input);
            }
            
            // Fallback: utwórz hasher dla bieżącej epoki
            let hasher = RandomXHasher::new(self.current_epoch);
            return hasher.hash(input);
        }
        
        #[cfg(not(all(feature = "randomx-ffi", target_pointer_width = "64")))]
        {
            // Fallback: SHA3-256 (not RandomX)
            use sha3::{Digest, Sha3_256};
            let mut h = Sha3_256::new();
            h.update(b"RANDOMX_FALLBACK");
            h.update(input);
            h.finalize().into()
        }
    }
    
    /// Update trust dla walidatora (główny algorytm RTT)
    pub fn update_validator_trust(&mut self, validator: NodeId, quality_q: Q) -> TrustScore {
        self.trust_graph.record_quality(validator, quality_q);
        self.trust_graph.update_trust(validator)
    }
    
    /// Update trust z Golden Trio (helper dla f64 → Q)
    pub fn update_validator_trust_f64(&mut self, validator: NodeId, quality_f64: f64) -> TrustScore {
        let quality_q = q_from_f64(quality_f64);
        self.update_validator_trust(validator, quality_q)
    }
    
    /// Get trust (Q)
    pub fn get_trust(&self, validator: &NodeId) -> TrustScore {
        self.trust_graph.get_trust(validator)
    }
    
    /// Get trust (f64, dla debug/display)
    pub fn get_trust_f64(&self, validator: &NodeId) -> f64 {
        self.trust_graph.get_trust_f64(validator)
    }
    
    /// Trust ranking (top N)
    pub fn get_top_validators(&self, n: usize) -> Vec<(NodeId, TrustScore)> {
        let mut ranking = self.trust_graph.get_ranking();
        ranking.truncate(n);
        ranking
    }
    
    /// Add vouch (with Q strength)
    pub fn add_vouch(&mut self, voucher: NodeId, vouchee: NodeId, strength_q: Q, epoch: u64) -> bool {
        let vouch = crate::rtt_trust_pro::Vouch {
            voucher,
            vouchee,
            strength: strength_q,
            created_at: epoch,
        };
        self.trust_graph.add_vouch(vouch)
    }
    
    /// Add vouch (helper z f64)
    pub fn add_vouch_f64(&mut self, voucher: NodeId, vouchee: NodeId, strength_f64: f64, epoch: u64) -> bool {
        let strength_q = q_from_f64(strength_f64);
        self.add_vouch(voucher, vouchee, strength_q, epoch)
    }
    
    /// Bootstrap nowego walidatora z vouchingiem
    pub fn bootstrap_validator(&mut self, validator: NodeId, vouchers: Vec<(NodeId, Q)>) -> TrustScore {
        crate::rtt_trust_pro::bootstrap_validator(&mut self.trust_graph, validator, vouchers)
    }
    
    /// Advance epoch
    pub fn advance_epoch(&mut self) {
        self.current_epoch += 1;
    }
    
    /// Current epoch
    pub fn current_epoch(&self) -> u64 {
        self.current_epoch
    }
    
    /// Export trust graph (DOT format dla Graphviz)
    pub fn export_graph_dot(&self) -> String {
        self.trust_graph.export_dot()
    }

    // === STARK (Winterfell) batch verify adapter ===
    /// Weryfikuje wszystkie **range STARK** w outpucie każdej transakcji.
    /// Nie dotyka podpisów (Falcon) ani reguł stanu.
    /// Zwraca (ile_poprawnych, ile_łącznie).
    #[inline]
    pub fn verify_block_stark(&self, txs: &[TransactionStark]) -> (u32, u32) {
        verify_stark_outputs_in_block(txs)
    }
}

impl Default for ConsensusPro {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper: Compute final weight (PoT + PoS + RandomX)
///
/// W = T^p_t × (1 + R)^p_r × S^p_s
pub fn compute_final_weight_pro(
    trust_q: Q,
    randomx_score: f64,
    stake_fraction_q: Q,
    power_trust: f64,
    power_randomx: f64,
    power_stake: f64,
) -> f64 {
    let trust_f = q_to_f64(trust_q);
    let stake_f = q_to_f64(stake_fraction_q);
    
    let w_trust = trust_f.powf(power_trust);
    let w_randomx = (1.0 + randomx_score).powf(power_randomx);
    let w_stake = stake_f.powf(power_stake);
    
    w_trust * w_randomx * w_stake
}

// =================== STARK (Winterfell) helpers ===================

/// Weryfikuje dowody STARK we wszystkich outputach transakcji.
/// Zwraca (valid_outputs, total_outputs).
#[inline]
pub fn verify_stark_outputs_in_block(transactions: &[TransactionStark]) -> (u32, u32) {
    let mut valid = 0u32;
    let mut total = 0u32;
    for tx in transactions {
        let (v, t) = tx.verify_all_proofs();
        valid += v;
        total += t;
    }
    (valid, total)
}

/// Zwraca ID transakcji, w których **jakikolwiek** output ma zły STARK.
#[inline]
pub fn find_txs_with_invalid_stark(transactions: &[TransactionStark]) -> Vec<Hash32> {
    let mut bad = Vec::new();
    for tx in transactions {
        let (v, t) = tx.verify_all_proofs();
        if v != t {
            bad.push(tx.id());
        }
    }
    bad
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_consensus_pro_basic() {
        let mut consensus = ConsensusPro::new();
        
        let alice = [1u8; 32];
        let bob = [2u8; 32];
        
        // Update trust
        let t_alice = consensus.update_validator_trust_f64(alice, 0.9);
        let t_bob = consensus.update_validator_trust_f64(bob, 0.7);
        
        assert!(q_to_f64(t_alice) > q_to_f64(t_bob));
        
        // Ranking
        let top = consensus.get_top_validators(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].0, alice);
    }
    
    #[test]
    fn test_vouching() {
        let mut consensus = ConsensusPro::new();
        
        let alice = [1u8; 32];
        let bob = [2u8; 32];
        
        // Alice ma wysoki trust
        consensus.update_validator_trust_f64(alice, 0.9);
        
        // Alice vouchuje Boba
        let ok = consensus.add_vouch_f64(alice, bob, 0.8, 0);
        assert!(ok);
        
        // Bob dostaje boost od vouchingu
        let t_bob = consensus.update_validator_trust_f64(bob, 0.5);
        assert!(q_to_f64(t_bob) > 0.5); // Powyżej bazowej jakości
    }
    
    #[test]
    fn test_randomx_fallback() {
        let mut consensus = ConsensusPro::new();
        
        let input = b"test block header";
        let hash = consensus.randomx_hash(input);
        
        // Deterministyczny (używa SHA3 fallback jeśli FFI niedostępne)
        let hash2 = consensus.randomx_hash(input);
        assert_eq!(hash, hash2);
    }
    
    #[test]
    fn test_epoch_advance() {
        let mut consensus = ConsensusPro::new();
        assert_eq!(consensus.current_epoch(), 0);
        
        consensus.advance_epoch();
        assert_eq!(consensus.current_epoch(), 1);
    }
    
    #[test]
    fn test_final_weight() {
        let trust_q = q_from_f64(0.9);
        let randomx_score = 0.8; // [0, 1]
        let stake_fraction_q = q_from_f64(0.1);
        
        let weight = compute_final_weight_pro(
            trust_q,
            randomx_score,
            stake_fraction_q,
            2.0, // power_trust
            1.5, // power_randomx
            1.0, // power_stake
        );
        
        // 0.9^2 × (1+0.8)^1.5 × 0.1^1 ≈ 0.81 × 2.15 × 0.1 ≈ 0.174
        assert!(weight > 0.15 && weight < 0.20, "Weight: {}", weight);
    }

    #[test]
    fn test_stark_block_verify_adapter() {
        use rand::RngCore;
        use crate::pq::tx_stark::{TxOutputStark, TransactionStark};
        use crate::kyber_kem::kyber_keypair;

        let (kyber_pk, _kyber_sk) = kyber_keypair();
        let mut b1 = [0u8; 32]; let mut b2 = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut b1);
        rand::thread_rng().fill_bytes(&mut b2);

        let o1 = TxOutputStark::new(123, &b1, [1u8;32], &kyber_pk);
        let o2 = TxOutputStark::new(456, &b2, [2u8;32], &kyber_pk);

        let tx = TransactionStark { inputs: vec![], outputs: vec![o1,o2], fee: 0, nonce: 0, timestamp: 0 };
        let (v,t) = super::verify_stark_outputs_in_block(&[tx]);
        assert_eq!(v,t);
    }
}
