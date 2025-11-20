#![forbid(unsafe_code)]

//! Deterministic validator weights for PRO consensus.
//!
//! Zero `f64`. Everything in Q32.32 + simple integers.
//!
//! Weight = w_trust * T + w_quality * Q + w_stake * S
//! where T, Q, S ∈ [0, 1] as Q32.32, and w_* are small integer coefficients.

use sha3::{Digest, Sha3_256};

use crate::node_id::NodeId;
use crate::rtt_pro::Q;

/// Weight in consensus (higher → better chance of leader / more fork-choice influence).
pub type Weight = u128;

/// Weight constants (can be pulled into config if desired).
/// Example here: trust has most weight, then last quality, then stake.
pub const W_TRUST: u128 = 4;
pub const W_QUALITY: u128 = 2;
pub const W_STAKE: u128 = 1;

/// Simple function normalizing Q32.32 to Weight.
#[inline]
fn q_to_weight(x: Q) -> u128 {
    x as u128
}

/// Computes final validator weight for consensus.
///
/// - `trust_q`   – RTT trust T(v) ∈ [0,1] (Q32.32)
/// - `quality_q` – last quality from GOLDEN_TRIO ∈ [0,1] (Q32.32)
/// - `stake_q`   – stake normalized to [0,1] (Q32.32)
///
/// Returns value comparable between validators (Weight).
pub fn compute_final_weight_q(
    trust_q: Q,
    quality_q: Q,
    stake_q: Q,
) -> Weight {
    let t = q_to_weight(trust_q);
    let q = q_to_weight(quality_q);
    let s = q_to_weight(stake_q);

    W_TRUST * t
        + W_QUALITY * q
        + W_STAKE * s
}

/// Deterministic leader selection.
///
/// - `beacon` – randomness from RandomX / VRF (Hash256 encoded as [u8;32])
/// - `validators` – list of `(NodeId, trust_q, quality_q, stake_q)`
///
/// Note: `beacon` is only used to mix weights deterministically.
pub fn select_leader_deterministic(
    beacon: [u8; 32],
    validators: &[(NodeId, Q, Q, Q)],
) -> Option<NodeId> {
    validators
        .iter()
        .map(|(id, trust_q, quality_q, stake_q)| {
            let w = compute_final_weight_q(*trust_q, *quality_q, *stake_q);

            // Hash: H("TT-LEADER.v1" || beacon || id || weight_be)
            let mut h = Sha3_256::new();
            h.update(b"TT-LEADER.v1");
            h.update(&beacon);
            h.update(id);
            h.update(&w.to_be_bytes());
            let digest = h.finalize();

            // Interpret hash as u128 (lowest 16B) – HIGHER value is better.
            let mut score_bytes = [0u8; 16];
            score_bytes.copy_from_slice(&digest[16..32]);
            let score = u128::from_be_bytes(score_bytes);

            (score, *id)
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, id)| id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rtt_pro::q_from_f64;

    #[test]
    fn higher_trust_gives_higher_weight() {
        let t1 = q_from_f64(0.9);
        let t2 = q_from_f64(0.5);
        let q = q_from_f64(0.7);
        let s = q_from_f64(0.4);

        let w1 = compute_final_weight_q(t1, q, s);
        let w2 = compute_final_weight_q(t2, q, s);

        assert!(w1 > w2);
    }

    #[test]
    fn select_leader_is_deterministic() {
        let beacon = [0x42u8; 32];
        let v1: NodeId = [1u8; 32];
        let v2: NodeId = [2u8; 32];

        let t_high = q_from_f64(0.9);
        let t_low = q_from_f64(0.3);
        let q_mid = q_from_f64(0.5);
        let s_mid = q_from_f64(0.4);

        let vals = vec![
            (v1, t_high, q_mid, s_mid),
            (v2, t_low, q_mid, s_mid),
        ];

        let leader1 = select_leader_deterministic(beacon, &vals).unwrap();
        let leader2 = select_leader_deterministic(beacon, &vals).unwrap();

        assert_eq!(leader1, leader2);
        assert_eq!(leader1, v1);
    }
}