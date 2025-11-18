#![forbid(unsafe_code)]

//! Deterministyczne wagi walidatorów dla konsensusu PRO.
//!
//! Zero `f64`. Wszystko w Q32.32 + prostych integerach.
//!
//! Waga = w_trust * T + w_quality * Q + w_stake * S
//! gdzie T, Q, S ∈ [0, 1] jako Q32.32, a w_* to małe całkowite współczynniki.

use sha3::{Digest, Sha3_256};

use crate::node_id::NodeId;
use crate::rtt_pro::Q;

/// Waga w konsensusie (większa → większa szansa na lidera / większy wpływ na fork-choice).
pub type Weight = u128;

/// Stałe wag (możesz je wyciągnąć do konfiga, jeśli chcesz).
/// Tutaj przykładowo: trust ma największe znaczenie, potem ostatnia jakość, potem stake.
pub const W_TRUST: u128 = 4;
pub const W_QUALITY: u128 = 2;
pub const W_STAKE: u128 = 1;

/// Prosta funkcja normalizująca Q32.32 do Weight.
#[inline]
fn q_to_weight(x: Q) -> u128 {
    x as u128
}

/// Wylicza finalną wagę walidatora na potrzeby konsensusu.
///
/// - `trust_q`   – RTT trust T(v) ∈ [0,1] (Q32.32)
/// - `quality_q` – ostatnia jakość z GOLDEN_TRIO ∈ [0,1] (Q32.32)
/// - `stake_q`   – stake znormalizowany do [0,1] (Q32.32)
///
/// Zwraca wartość porównywalną między walidatorami (Weight).
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

/// Deterministyczny wybór lidera.
///
/// - `beacon` – losowość z RandomX / VRF (Hash256 zakodowany jako [u8;32])
/// - `validators` – lista `(NodeId, trust_q, quality_q, stake_q)`
///
/// Uwaga: `beacon` używamy tylko do wymieszania wag w sposób deterministyczny.
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

            // Interpretujemy hash jako u128 (najmłodsze 16B) – im WYŻSZA wartość, tym lepiej.
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
