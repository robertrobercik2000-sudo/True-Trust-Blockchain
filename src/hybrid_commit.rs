//! Hybrid PQC Commitment Scheme
//! 
//! C_hybrid = r·G + v·H + fp·F
//! 
//! Where:
//! - G = Ristretto basepoint (blinding factor)
//! - H = cSHAKE256("TT-PEDERSEN-H") (value base, from bp.rs)
//! - F = cSHAKE256("TT-PQC-GEN") (PQC fingerprint base, NEW!)
//! - fp = Scalar::from_bytes_mod_order(KMAC(falcon_pk || mlkem_pk))
//!
//! Security:
//! - Binding: Requires knowledge of (v, r, fp) to open
//! - Hiding: Commitment is indistinguishable from random point
//! - PQ-resistant: fp binds to post-quantum public keys
//! - Backward compatible: If fp=0, reduces to classical Pedersen

#![forbid(unsafe_code)]

use curve25519_dalek::{
    constants::RISTRETTO_BASEPOINT_POINT as G,
    ristretto::RistrettoPoint,
    scalar::Scalar,
};
use tiny_keccak::{CShake, Hasher};
use sha3::{Shake256, digest::{Update, XofReader}};

pub type Hash32 = [u8; 32];

/// Domain separation for KMAC operations
pub const KMAC_KEY: &[u8] = b"agg-priv:v1";
pub const TAG_PQC_FP: &[u8] = b"PQC-FP.v1";

/* ============================================================================
 * Generator Points
 * ========================================================================== */

/// H generator (value base) - MUST match bp.rs for compatibility
/// Derived via cSHAKE256("TT-PEDERSEN-H")
#[inline(always)]
pub fn generator_H() -> RistrettoPoint {
    let mut cs = CShake::v256(b"TT-PEDERSEN-H", b"");
    let mut bytes = [0u8; 64];
    cs.finalize(&mut bytes);
    RistrettoPoint::from_uniform_bytes(&bytes)
}

/// F generator (PQC fingerprint base) - NEW for hybrid scheme
/// Derived via cSHAKE256("TT-PQC-GEN")
#[inline(always)]
pub fn generator_F() -> RistrettoPoint {
    let mut cs = CShake::v256(b"TT-PQC-GEN", b"");
    let mut bytes = [0u8; 64];
    cs.finalize(&mut bytes);
    RistrettoPoint::from_uniform_bytes(&bytes)
}

/* ============================================================================
 * PQC Fingerprint
 * ========================================================================== */

/// Compute PQC fingerprint from Falcon + ML-KEM public keys
/// 
/// fp = KMAC256(falcon_pk || mlkem_pk, domain="PQC-FP.v1")
pub fn pqc_fingerprint(falcon_pk: &[u8], mlkem_pk: &[u8]) -> Hash32 {
    use sha3::digest::{Update, ExtendableOutput, XofReader};
    
    let mut hasher = Shake256::default();
    Update::update(&mut hasher, b"KMAC256-DERIVE-v1");
    Update::update(&mut hasher, &(KMAC_KEY.len() as u64).to_le_bytes());
    Update::update(&mut hasher, KMAC_KEY);
    Update::update(&mut hasher, &(TAG_PQC_FP.len() as u64).to_le_bytes());
    Update::update(&mut hasher, TAG_PQC_FP);
    Update::update(&mut hasher, &(falcon_pk.len() as u64).to_le_bytes());
    Update::update(&mut hasher, falcon_pk);
    Update::update(&mut hasher, &(mlkem_pk.len() as u64).to_le_bytes());
    Update::update(&mut hasher, mlkem_pk);
    
    let mut reader = hasher.finalize_xof();
    let mut out = [0u8; 32];
    XofReader::read(&mut reader, &mut out);
    out
}

/// Convert fingerprint hash to scalar (mod order)
#[inline(always)]
pub fn fingerprint_to_scalar(fp: &Hash32) -> Scalar {
    Scalar::from_bytes_mod_order(*fp)
}

/* ============================================================================
 * Hybrid Commitment
 * ========================================================================== */

/// Opening for hybrid commitment
#[derive(Clone, Debug)]
pub struct HybridOpening {
    pub value: u64,
    pub blind: [u8; 32],
    pub pqc_fingerprint: Hash32,
}

/// Create hybrid commitment: C = r·G + v·H + fp·F
/// 
/// # Arguments
/// - `value`: Transaction value (u64)
/// - `blind`: 32-byte blinding factor (random)
/// - `pqc_fp`: PQC fingerprint from `pqc_fingerprint()`
/// 
/// # Returns
/// Compressed Ristretto point (32 bytes)
pub fn hybrid_commit(value: u64, blind: &[u8; 32], pqc_fp: &Hash32) -> [u8; 32] {
    let v = Scalar::from(value);
    let r = Scalar::from_bytes_mod_order(*blind);
    let fp = fingerprint_to_scalar(pqc_fp);
    
    let H = generator_H();
    let F = generator_F();
    
    let C = r * G + v * H + fp * F;
    C.compress().to_bytes()
}

/// Verify hybrid commitment opening
/// 
/// Returns true if C == r·G + v·H + fp·F
pub fn hybrid_verify(
    commitment: &[u8; 32],
    opening: &HybridOpening,
) -> bool {
    let expected = hybrid_commit(opening.value, &opening.blind, &opening.pqc_fingerprint);
    commitment == &expected
}

/* ============================================================================
 * Backward Compatibility
 * ========================================================================== */

/// Classical Pedersen commitment (fp = 0)
/// 
/// For backward compatibility with non-PQC notes
pub fn classical_commit(value: u64, blind: &[u8; 32]) -> [u8; 32] {
    let zero_fp = [0u8; 32];
    hybrid_commit(value, blind, &zero_fp)
}

/// Check if commitment is classical (fp = 0)
pub fn is_classical_commitment(pqc_fp: &Hash32) -> bool {
    pqc_fp == &[0u8; 32]
}

/* ============================================================================
 * Homomorphic Properties
 * ========================================================================== */

/// Add two hybrid commitments
/// 
/// C1 + C2 = (r1+r2)·G + (v1+v2)·H + (fp1+fp2)·F
/// 
/// NOTE: PQC fingerprints ADD (not useful for balance check,
/// but included for completeness)
pub fn hybrid_add(c1: &[u8; 32], c2: &[u8; 32]) -> Option<[u8; 32]> {
    use curve25519_dalek::ristretto::CompressedRistretto;
    
    let p1 = CompressedRistretto(*c1).decompress()?;
    let p2 = CompressedRistretto(*c2).decompress()?;
    
    Some((p1 + p2).compress().to_bytes())
}

/// Subtract commitments: C1 - C2
pub fn hybrid_sub(c1: &[u8; 32], c2: &[u8; 32]) -> Option<[u8; 32]> {
    use curve25519_dalek::ristretto::CompressedRistretto;
    
    let p1 = CompressedRistretto(*c1).decompress()?;
    let p2 = CompressedRistretto(*c2).decompress()?;
    
    Some((p1 - p2).compress().to_bytes())
}

/* ============================================================================
 * Balance Verification (for ZK guest)
 * ========================================================================== */

/// Verify balance equation in ZK:
/// Σ(C_in) == Σ(C_out) + C_fee
/// 
/// This checks:
/// Σ(r_in) == Σ(r_out) + r_fee (blinding conservation)
/// Σ(v_in) == Σ(v_out) + v_fee (value conservation)
/// Σ(fp_in) == Σ(fp_out) + fp_fee (fingerprint conservation)
/// 
/// For privacy, fp_fee should be 0 (fee is public, no PQC binding needed)
pub fn verify_balance_scalar(
    inputs: &[(u64, [u8; 32], Hash32)],   // (value, blind, fp)
    outputs: &[(u64, [u8; 32], Hash32)],
    fee: &(u64, [u8; 32], Hash32),
) -> bool {
    let mut sum_v_in = Scalar::ZERO;
    let mut sum_r_in = Scalar::ZERO;
    let mut sum_fp_in = Scalar::ZERO;
    
    for (v, r, fp) in inputs {
        sum_v_in += Scalar::from(*v);
        sum_r_in += Scalar::from_bytes_mod_order(*r);
        sum_fp_in += fingerprint_to_scalar(fp);
    }
    
    let mut sum_v_out = Scalar::ZERO;
    let mut sum_r_out = Scalar::ZERO;
    let mut sum_fp_out = Scalar::ZERO;
    
    for (v, r, fp) in outputs {
        sum_v_out += Scalar::from(*v);
        sum_r_out += Scalar::from_bytes_mod_order(*r);
        sum_fp_out += fingerprint_to_scalar(fp);
    }
    
    let (v_fee, r_fee, fp_fee) = fee;
    sum_v_out += Scalar::from(*v_fee);
    sum_r_out += Scalar::from_bytes_mod_order(*r_fee);
    sum_fp_out += fingerprint_to_scalar(fp_fee);
    
    // All three components must balance
    sum_v_in == sum_v_out && 
    sum_r_in == sum_r_out && 
    sum_fp_in == sum_fp_out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;
    use rand::RngCore;

    #[test]
    fn test_generators_deterministic() {
        let H1 = generator_H();
        let H2 = generator_H();
        assert_eq!(H1, H2, "H generator must be deterministic");
        
        let F1 = generator_F();
        let F2 = generator_F();
        assert_eq!(F1, F2, "F generator must be deterministic");
        
        assert_ne!(H1, F1, "H and F must be independent");
        assert_ne!(H1, G, "H and G must be independent");
        assert_ne!(F1, G, "F and G must be independent");
    }

    #[test]
    fn test_pqc_fingerprint_deterministic() {
        let falcon_pk = [0x42u8; 897];
        let mlkem_pk = [0x99u8; 1184];
        
        let fp1 = pqc_fingerprint(&falcon_pk, &mlkem_pk);
        let fp2 = pqc_fingerprint(&falcon_pk, &mlkem_pk);
        
        assert_eq!(fp1, fp2, "Fingerprint must be deterministic");
    }

    #[test]
    fn test_hybrid_commit_roundtrip() {
        let mut blind = [0u8; 32];
        OsRng.fill_bytes(&mut blind);
        
        let falcon_pk = [0x42u8; 897];
        let mlkem_pk = [0x99u8; 1184];
        let pqc_fp = pqc_fingerprint(&falcon_pk, &mlkem_pk);
        
        let value = 1000u64;
        let commitment = hybrid_commit(value, &blind, &pqc_fp);
        
        let opening = HybridOpening {
            value,
            blind,
            pqc_fingerprint: pqc_fp,
        };
        
        assert!(hybrid_verify(&commitment, &opening), "Opening must verify");
        
        // Wrong value should fail
        let wrong_opening = HybridOpening {
            value: 999,
            blind,
            pqc_fingerprint: pqc_fp,
        };
        assert!(!hybrid_verify(&commitment, &wrong_opening), "Wrong value must fail");
    }

    #[test]
    fn test_classical_compatibility() {
        let mut blind = [0u8; 32];
        OsRng.fill_bytes(&mut blind);
        let value = 500u64;
        
        let classical = classical_commit(value, &blind);
        let zero_fp = [0u8; 32];
        let hybrid = hybrid_commit(value, &blind, &zero_fp);
        
        assert_eq!(classical, hybrid, "Classical must equal hybrid with fp=0");
    }

    #[test]
    fn test_balance_conservation() {
        let mut r1 = [0u8; 32]; OsRng.fill_bytes(&mut r1);
        let mut r2 = [0u8; 32]; OsRng.fill_bytes(&mut r2);
        let mut r_out = [0u8; 32]; OsRng.fill_bytes(&mut r_out);
        
        let fp1 = pqc_fingerprint(&[1u8; 897], &[2u8; 1184]);
        let fp2 = pqc_fingerprint(&[3u8; 897], &[4u8; 1184]);
        let fp_out = pqc_fingerprint(&[5u8; 897], &[6u8; 1184]);
        
        // Set up balanced transaction: 100 + 200 = 250 + 50 (fee)
        let inputs = vec![
            (100u64, r1, fp1),
            (200u64, r2, fp2),
        ];
        
        // Compute r_fee such that Σr_in = r_out + r_fee
        let r_in_sum = Scalar::from_bytes_mod_order(r1) + Scalar::from_bytes_mod_order(r2);
        let r_fee_scalar = r_in_sum - Scalar::from_bytes_mod_order(r_out);
        let r_fee = r_fee_scalar.to_bytes();
        
        // Compute fp_fee such that Σfp_in = fp_out + fp_fee
        let fp_in_sum = fingerprint_to_scalar(&fp1) + fingerprint_to_scalar(&fp2);
        let fp_fee_scalar = fp_in_sum - fingerprint_to_scalar(&fp_out);
        let fp_fee = fp_fee_scalar.to_bytes();
        
        let outputs = vec![
            (250u64, r_out, fp_out),
        ];
        
        let fee = (50u64, r_fee, fp_fee);
        
        assert!(verify_balance_scalar(&inputs, &outputs, &fee), "Balance must verify");
        
        // Wrong value should fail
        let bad_fee = (51u64, r_fee, fp_fee);
        assert!(!verify_balance_scalar(&inputs, &outputs, &bad_fee), "Wrong fee must fail");
    }

    #[test]
    fn test_homomorphic_add() {
        let mut r1 = [0u8; 32]; OsRng.fill_bytes(&mut r1);
        let mut r2 = [0u8; 32]; OsRng.fill_bytes(&mut r2);
        
        let fp1 = pqc_fingerprint(&[1u8; 897], &[2u8; 1184]);
        let fp2 = pqc_fingerprint(&[3u8; 897], &[4u8; 1184]);
        
        let c1 = hybrid_commit(100, &r1, &fp1);
        let c2 = hybrid_commit(200, &r2, &fp2);
        let c_sum = hybrid_add(&c1, &c2).unwrap();
        
        // Compute expected sum manually
        let r_sum_scalar = Scalar::from_bytes_mod_order(r1) + Scalar::from_bytes_mod_order(r2);
        let fp_sum_scalar = fingerprint_to_scalar(&fp1) + fingerprint_to_scalar(&fp2);
        
        let r_sum = r_sum_scalar.to_bytes();
        let fp_sum = fp_sum_scalar.to_bytes();
        
        let c_expected = hybrid_commit(300, &r_sum, &fp_sum);
        
        assert_eq!(c_sum, c_expected, "Homomorphic addition must work");
    }
}
