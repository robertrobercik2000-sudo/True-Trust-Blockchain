#![forbid(unsafe_code)]

//! Bulletproofs verification for 64-bit range proofs
//! Production-grade implementation using curve25519-dalek

#![allow(non_snake_case)]
#![allow(dead_code)]

use curve25519_dalek::{
    constants::RISTRETTO_BASEPOINT_POINT as G,
    ristretto::{CompressedRistretto, RistrettoPoint},
    scalar::Scalar,
    traits::MultiscalarMul,
};
use merlin::Transcript;
use tiny_keccak::CShake;
use tiny_keccak::Hasher;

pub type VerifyResult<T> = core::result::Result<T, &'static str>;

pub const BP_LABEL:      &[u8] = b"TT-BULLETPROOF64";
pub const BP_LABEL_GVEC: &[u8] = b"TT-BP-GVEC";
pub const BP_LABEL_HVEC: &[u8] = b"TT-BP-HVEC";
pub const BP_LABEL_Q:    &[u8] = b"TT-BP-Q";

pub const N_BITS: usize = 64;

#[derive(Clone, Debug)]
pub struct IppProof {
    pub L_vec: Vec<CompressedRistretto>,
    pub R_vec: Vec<CompressedRistretto>,
    pub a: Scalar,
    pub b: Scalar,
}

#[derive(Clone, Debug)]
pub struct RangeProof {
    pub A:     CompressedRistretto,
    pub S:     CompressedRistretto,
    pub T1:    CompressedRistretto,
    pub T2:    CompressedRistretto,
    pub taux:  Scalar,
    pub mu:    Scalar,
    pub t_hat: Scalar,
    pub ipp:   IppProof,
}

/* ---------- helpers ---------- */

fn decompress_point(c: &CompressedRistretto) -> VerifyResult<RistrettoPoint> {
    c.decompress().ok_or("bad compressed point")
}

fn challenge_scalar(t: &mut Transcript, label: &'static [u8]) -> Scalar {
    let mut buf = [0u8; 64];
    t.challenge_bytes(label, &mut buf);
    Scalar::from_bytes_mod_order_wide(&buf)
}

fn hash_to_point(domain: &'static [u8], idx: u64) -> RistrettoPoint {
    let mut tt = Transcript::new(domain);
    tt.append_u64(b"idx", idx);
    let mut bytes = [0u8; 64];
    tt.challenge_bytes(b"point", &mut bytes);
    RistrettoPoint::from_uniform_bytes(&bytes)
}

fn fixed_generators() -> (Vec<RistrettoPoint>, Vec<RistrettoPoint>, RistrettoPoint) {
    let mut G_vec = Vec::with_capacity(N_BITS);
    let mut H_vec = Vec::with_capacity(N_BITS);
    for i in 0..(N_BITS as u64) {
        G_vec.push(hash_to_point(BP_LABEL_GVEC, i));
        H_vec.push(hash_to_point(BP_LABEL_HVEC, i));
    }
    let Q = hash_to_point(BP_LABEL_Q, 0);
    (G_vec, H_vec, Q)
}

fn delta_yz(y: &Scalar, z: &Scalar) -> Scalar {
    let mut y_pow = Scalar::ONE;
    let mut sum_y = Scalar::ZERO;
    for _ in 0..N_BITS { sum_y += y_pow; y_pow *= *y; }
    let mut two_pow_sum = Scalar::ZERO;
    let mut cur = Scalar::ONE;
    let two = Scalar::from(2u64);
    for _ in 0..N_BITS { two_pow_sum += cur; cur *= two; }
    (*z - (*z * *z)) * sum_y - *z * *z * *z * two_pow_sum
}

fn log2_exact(n: usize) -> VerifyResult<usize> {
    let mut k = 0usize; let mut x = n;
    while x > 1 { if x & 1 != 0 { return Err("n not power of two"); } x >>= 1; k += 1; }
    Ok(k)
}

fn y_powers_and_inverses(y: &Scalar) -> (Vec<Scalar>, Vec<Scalar>) {
    let mut y_pows = Vec::with_capacity(N_BITS);
    let mut cur = Scalar::ONE;
    for _ in 0..N_BITS { y_pows.push(cur); cur *= *y; }
    let y_inv = y.invert();
    let mut y_inv_pows = Vec::with_capacity(N_BITS);
    cur = Scalar::ONE;
    for _ in 0..N_BITS { y_inv_pows.push(cur); cur *= y_inv; }
    (y_pows, y_inv_pows)
}

fn two_powers() -> Vec<Scalar> {
    let mut out = Vec::with_capacity(N_BITS);
    let mut cur = Scalar::ONE; let two = Scalar::from(2u64);
    for _ in 0..N_BITS { out.push(cur); cur *= two; }
    out
}

fn ipp_s_vector(us: &[Scalar]) -> (Vec<Scalar>, Vec<Scalar>) {
    let k = us.len();
    let u_inv: Vec<Scalar> = us.iter().map(|u| u.invert()).collect();
    let mut s = Vec::with_capacity(N_BITS);
    for i in 0..N_BITS {
        let mut acc = Scalar::ONE;
        for j in 0..k {
            let bit = ((i >> (k - 1 - j)) & 1) as u8;
            acc *= if bit == 1 { us[j] } else { u_inv[j] };
        }
        s.push(acc);
    }
    let s_inv: Vec<Scalar> = s.iter().map(|x| x.invert()).collect();
    (s, s_inv)
}

/// Verify 64-bit range proof for given commitment point V
pub fn verify_range_proof_64(
    proof: &RangeProof,
    V_bytes: [u8; 32],
    H_pedersen: RistrettoPoint,
) -> VerifyResult<()> {
    let A  = decompress_point(&proof.A)?;
    let S  = decompress_point(&proof.S)?;
    let T1 = decompress_point(&proof.T1)?;
    let T2 = decompress_point(&proof.T2)?;
    let V  = decompress_point(&CompressedRistretto(V_bytes))?;

    let mut t = Transcript::new(BP_LABEL);
    t.append_message(b"V",  &V.compress().to_bytes());
    t.append_message(b"A",  proof.A.as_bytes());
    t.append_message(b"S",  proof.S.as_bytes());
    let y = challenge_scalar(&mut t, b"y");
    let z = challenge_scalar(&mut t, b"z");
    t.append_message(b"T1", proof.T1.as_bytes());
    t.append_message(b"T2", proof.T2.as_bytes());
    let x = challenge_scalar(&mut t, b"x");

    let lhs = (proof.t_hat * H_pedersen) + (proof.taux * G);
    let rhs = ((z * z) * V) + (delta_yz(&y, &z) * H_pedersen) + (x * T1) + ((x * x) * T2);
    if lhs != rhs { return Err("poly check failed"); }

    let (Gi, Hi, Q) = fixed_generators();
    let (y_pows, y_inv_pows) = y_powers_and_inverses(&y);
    let two_pows = two_powers();

    let mut scalars_g = Vec::with_capacity(N_BITS);
    scalars_g.resize(N_BITS, -z);
    let P_g = RistrettoPoint::multiscalar_mul(scalars_g.iter().cloned(), Gi.iter().cloned());

    let mut scalars_h = Vec::with_capacity(N_BITS);
    for i in 0..N_BITS {
        let term = z * y_pows[i] + (z * z) * two_pows[i];
        scalars_h.push(term);
    }
    let P_h = RistrettoPoint::multiscalar_mul(scalars_h.iter().cloned(), Hi.iter().cloned());

    let mut P = A + x * S + P_g + P_h - (proof.mu * H_pedersen);

    let lg = proof.ipp.L_vec.len();
    if lg != proof.ipp.R_vec.len() { return Err("L/R len mismatch"); }
    if lg != log2_exact(N_BITS)? { return Err("L/R len != log2(n)"); }

    let mut us: Vec<Scalar> = Vec::with_capacity(lg);
    for k in 0..lg {
        t.append_message(b"L", proof.ipp.L_vec[k].as_bytes());
        t.append_message(b"R", proof.ipp.R_vec[k].as_bytes());
        us.push(challenge_scalar(&mut t, b"u"));
    }
    for k in 0..lg {
        let uk = us[k];
        let uk2 = uk * uk;
        let ukm2 = uk.invert();
        let ukm2 = ukm2 * ukm2;
        let Lk = decompress_point(&proof.ipp.L_vec[k])?;
        let Rk = decompress_point(&proof.ipp.R_vec[k])?;
        P = P + uk2 * Lk + ukm2 * Rk;
    }

    let (s, s_inv) = ipp_s_vector(&us);

    let ab = proof.ipp.a * proof.ipp.b;
    let g_scalars: Vec<Scalar> = s.iter().map(|si| proof.ipp.a * *si).collect();
    let h_scalars: Vec<Scalar> = (0..N_BITS)
        .map(|i| proof.ipp.b * s_inv[i] * y_inv_pows[i])
        .collect();

    let msm_g = RistrettoPoint::multiscalar_mul(g_scalars.iter().cloned(), Gi.iter().cloned());
    let msm_h = RistrettoPoint::multiscalar_mul(h_scalars.iter().cloned(), Hi.iter().cloned());
    let left = msm_g + msm_h + ab * Q;

    if left != P { return Err("ipp check failed"); }
    if proof.t_hat != ab { return Err("t_hat != a*b"); }
    Ok(())
}

/* ---------- parser + H pedersen ---------- */

fn read32(src: &[u8], off: &mut usize) -> Result<[u8; 32], &'static str> {
    if *off + 32 > src.len() { return Err("eof"); }
    let mut out = [0u8; 32];
    out.copy_from_slice(&src[*off..*off + 32]);
    *off += 32;
    Ok(out)
}

fn scalar_from_32(le: [u8; 32]) -> Result<Scalar, &'static str> {
    let ct = Scalar::from_canonical_bytes(le);
    if bool::from(ct.is_some()) {
        Ok(ct.unwrap())
    } else {
        Err("bad scalar canonical")
    }
}

pub fn parse_dalek_range_proof_64(bytes: &[u8]) -> Result<RangeProof, &'static str> {
    const LG: usize = 6;
    const EXPECTED: usize = 672;
    if bytes.len() != EXPECTED { return Err("bad length"); }
    let mut off = 0usize;

    let A   = CompressedRistretto(read32(bytes, &mut off)?);
    let S   = CompressedRistretto(read32(bytes, &mut off)?);
    let T1  = CompressedRistretto(read32(bytes, &mut off)?);
    let T2  = CompressedRistretto(read32(bytes, &mut off)?);

    let t_hat = scalar_from_32(read32(bytes, &mut off)?)?;
    let taux  = scalar_from_32(read32(bytes, &mut off)?)?;
    let mu    = scalar_from_32(read32(bytes, &mut off)?)?;

    let mut L_vec = Vec::with_capacity(LG);
    let mut R_vec = Vec::with_capacity(LG);
    for _ in 0..LG { L_vec.push(CompressedRistretto(read32(bytes, &mut off)?)); }
    for _ in 0..LG { R_vec.push(CompressedRistretto(read32(bytes, &mut off)?)); }

    let a = scalar_from_32(read32(bytes, &mut off)?)?;
    let b = scalar_from_32(read32(bytes, &mut off)?)?;

    if off != bytes.len() { return Err("trailing bytes"); }

    Ok(RangeProof { A, S, T1, T2, taux, mu, t_hat, ipp: IppProof{ L_vec, R_vec, a, b } })
}

/// Unified H pedersen for host: cSHAKE256("TT-PEDERSEN-H")
pub fn derive_H_pedersen() -> RistrettoPoint {
    let mut cs = CShake::v256(b"TT-PEDERSEN-H", b"");
    let mut bytes = [0u8; 64];
    cs.finalize(&mut bytes);
    RistrettoPoint::from_uniform_bytes(&bytes)
}

/// Pedersen commitment C(v,r) = r·G + v·H
pub fn pedersen_commit_bytes(value: u64, blind32: [u8; 32], H_pedersen: RistrettoPoint) -> [u8; 32] {
    let r = Scalar::from_bytes_mod_order(blind32);
    let v = Scalar::from(value);
    let C = r * G + v * H_pedersen;
    C.compress().to_bytes()
}

/// Verify proof directly against C_out (binding V==C_out)
pub fn verify_bound_range_proof_64_bytes(
    proof_bytes: &[u8],
    C_out_bytes: [u8;32],
    H_pedersen: RistrettoPoint
) -> VerifyResult<()> {
    let rp = parse_dalek_range_proof_64(proof_bytes)?;
    verify_range_proof_64(&rp, C_out_bytes, H_pedersen)
}
