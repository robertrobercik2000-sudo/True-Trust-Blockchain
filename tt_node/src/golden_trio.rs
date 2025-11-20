// src/golden_trio.rs
#![forbid(unsafe_code)]

//! GOLDEN TRIO CONSENSUS - Quality & Slashing Model
//!
//! Matematyka z GOLDEN_TRIO_CONSENSUS.md
//!
//! Wzór high-level (ANALITYCZNY, f64 – NIE na hot-path konsensusu):
//!
//! ```text
//! W_final = T^1.0 × (1+R)^0.5 × S^0.8
//!
//! gdzie:
//!   T = Σ αᵢ·Tᵢ  (6 komponentów trust/quality)
//!   R = solved/expected (RandomX mining)
//!   S = stake_eff / stake_total (locked stake)
//! ```
//!
//! W samym konsensusie używamy wersji integerowej z `consensus_weights` +
//! `ConsensusPro`; ten moduł służy do LICZENIA quality/kar, a nie do
//! leader-selection (tam jest Q32.32).

use crate::rtt_pro::{Q, ONE_Q};

/// Hard Trust / Quality Metrics (6 components)
#[derive(Clone, Debug, Default)]
pub struct HardTrustMetrics {
    // T1: Block Production
    pub blocks_produced: u64,
    pub target_blocks: u64,
    
    // T2: Proof Generation
    pub bulletproofs_count: u64,
    pub bulletproofs_valid: u64,
    pub zk_proofs_count: u64,
    pub zk_proofs_valid: u64,
    pub pow_proofs_count: u64,
    pub pow_proofs_valid: u64,
    
    // T3: Uptime
    pub uptime_slots: u64,
    pub eligible_slots: u64,
    
    // T4: Stake Lock
    pub lock_days: u32,
    pub stake_variance: f64,  // Consistency of stake (tu tylko do analizy)
    
    // T5: Fees
    pub fees_collected: f64,
    pub expected_fees: f64,
    
    // T6: Network
    pub active_peers: u64,
    pub target_peers: u64,
    pub avg_propagation_ms: u64,
    pub max_propagation_ms: u64,
}

/// Trust / quality component weights (α₁...α₆, suma ~ 1.0)
#[derive(Clone, Debug)]
pub struct TrustWeights {
    pub blocks: f64,      // α₁ = 0.30
    pub proofs: f64,      // α₂ = 0.25
    pub uptime: f64,      // α₃ = 0.20
    pub stake_lock: f64,  // α₄ = 0.10
    pub fees: f64,        // α₅ = 0.10
    pub network: f64,     // α₆ = 0.05
}

impl Default for TrustWeights {
    fn default() -> Self {
        Self {
            blocks: 0.30,
            proofs: 0.25,
            uptime: 0.20,
            stake_lock: 0.10,
            fees: 0.10,
            network: 0.05,
        }
    }
}

/// Proof sub-weights (dla T2, suma = 1.0)
#[derive(Clone, Debug)]
pub struct ProofWeights {
    pub bp: f64,   // w_bp = 0.4
    pub zk: f64,   // w_zk = 0.4
    pub pow: f64,  // w_pow = 0.2
}

impl Default for ProofWeights {
    fn default() -> Self {
        Self {
            bp: 0.4,
            zk: 0.4,
            pow: 0.2,
        }
    }
}

/// Stake lock time multiplier
///
/// lock(t) = 1 + k × ln(1 + t/t_base)
///
/// k = 0.5, t_base = 30 dni
pub fn stake_lock_multiplier(lock_days: u32) -> f64 {
    const K: f64 = 0.5;
    const T_BASE: f64 = 30.0;
    
    1.0 + K * (1.0 + (lock_days as f64 / T_BASE)).ln()
}

/// Compute "hard" trust / quality z 6 komponentów (f64, off-chain / off-path)
///
/// T_total = α₁·T_blocks + α₂·T_proofs + α₃·T_uptime + 
///           α₄·T_stake + α₅·T_fees + α₆·T_network
///
/// Zwraca T ∈ [0.0, 1.0]
pub fn compute_hard_trust(
    metrics: &HardTrustMetrics,
    trust_weights: &TrustWeights,
    proof_weights: &ProofWeights,
) -> f64 {
    // T1: Block Production
    let t_blocks = if metrics.target_blocks > 0 {
        (metrics.blocks_produced as f64 / metrics.target_blocks as f64).min(1.0)
    } else {
        0.0
    };
    
    // T2: Proof Generation
    let bp_ratio = if metrics.bulletproofs_count > 0 {
        metrics.bulletproofs_valid as f64 / metrics.bulletproofs_count as f64
    } else {
        0.0
    };
    let zk_ratio = if metrics.zk_proofs_count > 0 {
        metrics.zk_proofs_valid as f64 / metrics.zk_proofs_count as f64
    } else {
        0.0
    };
    let pow_ratio = if metrics.pow_proofs_count > 0 {
        metrics.pow_proofs_valid as f64 / metrics.pow_proofs_count as f64
    } else {
        0.0
    };
    let t_proofs = proof_weights.bp * bp_ratio +
                   proof_weights.zk * zk_ratio +
                   proof_weights.pow * pow_ratio;
    
    // T3: Uptime
    let t_uptime = if metrics.eligible_slots > 0 {
        metrics.uptime_slots as f64 / metrics.eligible_slots as f64
    } else { 0.0 };
    
    // T4: Stake Lock
    let lock_mult = stake_lock_multiplier(metrics.lock_days);
    let t_stake = (lock_mult / 4.0).min(1.0); // normalizacja ~4x
    
    // T5: Fee Collection
    let t_fees = if metrics.expected_fees > 0.0 {
        (metrics.fees_collected / metrics.expected_fees).min(1.0)
    } else { 0.0 };
    
    // T6: Network
    let peer_score = if metrics.target_peers > 0 {
        metrics.active_peers as f64 / metrics.target_peers as f64
    } else { 0.0 };
    let propagation_score = if metrics.max_propagation_ms > 0 {
        1.0 - (metrics.avg_propagation_ms as f64 / metrics.max_propagation_ms as f64)
    } else { 0.0 };
    let t_network = 0.5 * peer_score + 0.5 * propagation_score;
    
    let t_total = trust_weights.blocks * t_blocks +
                  trust_weights.proofs * t_proofs +
                  trust_weights.uptime * t_uptime +
                  trust_weights.stake_lock * t_stake +
                  trust_weights.fees * t_fees +
                  trust_weights.network * t_network;
    
    t_total.clamp(0.0, 1.0)
}

/// Aktualizacja trustu w Q32.32 z „miękkim” decajem.
///
/// T(t+1) = β·T(t) + (1-β)·T_computed
pub fn update_trust_with_decay(
    current_trust_q: Q,
    computed_trust: f64,
    decay_beta: f64,
) -> Q {
    let current_f = (current_trust_q as f64) / (ONE_Q as f64);
    let new_f = decay_beta * current_f + (1.0 - decay_beta) * computed_trust;
    let new_q = (new_f * (ONE_Q as f64)) as u64;
    new_q.clamp(0, ONE_Q)
}

/// High-level waga analityczna (f64, tylko do symulacji / researchu)
///
/// W_final = T^p_trust × (1+R)^p_randomx × S^p_stake
pub fn compute_final_weight_f64(
    trust: f64,         // T ∈ [0,1]
    randomx_score: f64, // R = solved/expected
    stake_fraction: f64,// S ∈ [0,1]
    powers: &PowerParams,
) -> f64 {
    let w_trust = trust.powf(powers.trust);
    let w_randomx = (1.0 + randomx_score).powf(powers.randomx);
    let w_stake = stake_fraction.powf(powers.stake);
    w_trust * w_randomx * w_stake
}

/// Parametry potęg dla powyższego wzoru (tylko analitycznie)
#[derive(Clone, Debug)]
pub struct PowerParams {
    pub trust: f64,      // p_trust = 1.0
    pub randomx: f64,    // p_randomx = 0.5
    pub stake: f64,      // p_stake = 0.8
}

impl Default for PowerParams {
    fn default() -> Self {
        Self {
            trust: 1.0,
            randomx: 0.5,
            stake: 0.8,
        }
    }
}

/// Efektywny stake z mnożnikiem lock-time (analitycznie, f64)
pub fn compute_effective_stake(stake: u64, lock_days: u32) -> f64 {
    let mult = stake_lock_multiplier(lock_days);
    (stake as f64) * mult
}

/// Minimalny stake przy wzroście liczby walidatorów.
///
/// min_stake = BASE × (1 + log₁₀(validators / 100))
pub fn compute_min_stake(total_validators: u64, base_stake: u64) -> u64 {
    let growth_factor = if total_validators > 100 {
        ((total_validators as f64) / 100.0).log10()
    } else {
        0.0
    };
    base_stake + (base_stake as f64 * growth_factor) as u64
}

/// Kwota slasha (f64 na wejściu, finalnie u64).
///
/// slash = base_penalty × severity × stake
pub fn compute_slash_amount(
    stake: u64,
    severity: u32,
    base_penalty: f64,
) -> u64 {
    let slash_ratio = base_penalty * (severity as f64);
    ((stake as f64) * slash_ratio.min(1.0)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stake_lock_multiplier_examples() {
        assert!((stake_lock_multiplier(0) - 1.0).abs() < 0.01);
        assert!((stake_lock_multiplier(30) - 1.347).abs() < 0.01);
        assert!((stake_lock_multiplier(90) - 1.693).abs() < 0.01);
        assert!((stake_lock_multiplier(180) - 1.973).abs() < 0.01);
        assert!((stake_lock_multiplier(365) - 2.282).abs() < 0.01);
    }

    #[test]
    fn test_compute_hard_trust_perfect() {
        let metrics = HardTrustMetrics {
            blocks_produced: 100,
            target_blocks: 100,
            bulletproofs_valid: 50,
            bulletproofs_count: 50,
            zk_proofs_valid: 25,
            zk_proofs_count: 25,
            pow_proofs_valid: 10,
            pow_proofs_count: 10,
            uptime_slots: 1000,
            eligible_slots: 1000,
            lock_days: 365,
            stake_variance: 0.0,
            fees_collected: 100.0,
            expected_fees: 100.0,
            active_peers: 20,
            target_peers: 20,
            avg_propagation_ms: 50,
            max_propagation_ms: 1000,
        };
        
        let trust_weights = TrustWeights::default();
        let proof_weights = ProofWeights::default();
        let trust = compute_hard_trust(&metrics, &trust_weights, &proof_weights);
        assert!(trust > 0.9, "Trust: {}", trust);
    }

    #[test]
    fn test_min_stake_scaling() {
        assert_eq!(compute_min_stake(100, 100_000), 100_000);
        assert_eq!(compute_min_stake(1_000, 100_000), 200_000);
        assert_eq!(compute_min_stake(10_000, 100_000), 300_000);
    }

    #[test]
    fn test_slashing() {
        let stake = 100_000;
        assert_eq!(compute_slash_amount(stake, 1, 0.01), 1_000);
        assert_eq!(compute_slash_amount(stake, 10, 0.01), 10_000);
        assert_eq!(compute_slash_amount(stake, 100, 0.01), 100_000);
    }
}
