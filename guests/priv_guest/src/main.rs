#![no_std]
#![no_main]
extern crate alloc;

use alloc::vec::Vec;
use risc0_zkvm::guest::env;
use serde::{Deserialize, Serialize};
use tiny_keccak::{Hasher, Kmac};

use curve25519_dalek::constants::RISTRETTO_BASEPOINT_POINT as G;
use curve25519_dalek::ristretto::RistrettoPoint;
use curve25519_dalek::scalar::Scalar;

type Hash32 = [u8; 32];

/* ================== Konfiguracja / domeny ================== */

const KMAC_KEY: &[u8] = b"agg-priv:v1";
const TAG_LEAF: &[u8] = b"LEAF.v1";
const TAG_NODE: &[u8] = b"NODE.v1";
const TAG_HINT: &[u8] = b"HINT.v1";
const TAG_NF: &[u8]   = b"NF.v1";

/* ================== I/O (with PQC fingerprints) ================== */

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MerkleProof { pub index: u32, pub siblings: Vec<Hash32> }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivInput {
    pub old_notes_root: Hash32,
    pub ins:  Vec<InPublic>,
    pub outs: Vec<OutPublic>,
    pub fee_commit: [u8; 32],
    pub network_id: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InPublic {
    pub note_commit_hash: Hash32,
    pub nullifier: Hash32,
    pub proof: MerkleProof,
    // ✅ NEW: PQC fingerprint (public input, NOT verified in ZK)
    pub pqc_fingerprint: Hash32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutPublic {
    pub note_commit_point: [u8; 32],
    pub enc_hint: Vec<u8>,
    // ✅ NEW: PQC fingerprint for recipient
    pub pqc_fingerprint: Hash32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivWitness {
    pub in_openings:  Vec<InOpen>,
    pub out_openings: Vec<OutOpen>,
    pub fee_value: u64,
    pub fee_blind: [u8; 32],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InOpen {
    pub value: u64,
    pub blind: [u8; 32],
    pub spend_key: [u8; 32],
    pub leaf_index: u32,
    pub note_commit_point: [u8; 32],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutOpen {
    pub value: u64,
    pub blind: [u8; 32],
    pub note_commit_point: [u8; 32],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivClaim {
    pub nullifiers: Vec<Hash32>,
    pub outputs:    Vec<[u8; 32]>,
    pub enc_hints:  Vec<[u8; 32]>,
    pub fee_commit: [u8; 32],
    // ✅ NEW: PQC fingerprints for host verification
    pub pqc_fingerprints_in: Vec<Hash32>,
    pub pqc_fingerprints_out: Vec<Hash32>,
}

/* ================== KMAC + Pedersen (CLASSICAL) ================== */

#[inline(always)]
fn kmac32(key: &[u8], custom: &[u8], parts: &[&[u8]]) -> Hash32 {
    let mut km = Kmac::v256(key, custom);
    for p in parts { km.update(p); }
    let mut o = [0u8; 32];
    km.finalize(&mut o);
    o
}

#[inline(always)]
fn h_leaf(pt: &[u8; 32]) -> Hash32 { kmac32(KMAC_KEY, TAG_LEAF, &[pt]) }

#[inline(always)]
fn h_pair(l: &Hash32, r: &Hash32) -> Hash32 { kmac32(KMAC_KEY, TAG_NODE, &[l, r]) }

/// ⚠️ H generator MUST match host bp.rs (cSHAKE256("TT-PEDERSEN-H"))
const H_BYTES: [u8; 64] = [
    0x36, 0x11, 0x4B, 0x51, 0xF9, 0x1E, 0x24, 0x0C, 0x19, 0x76, 0x6C, 0x67, 0x74, 0x48, 0x21, 0x16,
    0x6C, 0x2E, 0x51, 0x7E, 0x54, 0x65, 0x7C, 0x10, 0xC3, 0x2F, 0xC5, 0x0A, 0x31, 0x82, 0x42, 0x19,
    0x11, 0x10, 0x2B, 0x23, 0x4D, 0xD4, 0x36, 0xD7, 0xA8, 0x67, 0x65, 0x4F, 0x2A, 0x73, 0x04, 0x56,
    0x49, 0xC4, 0x2A, 0x4F, 0x93, 0x7D, 0x3B, 0x5E, 0x1A, 0x31, 0xF4, 0x9A, 0x53, 0xAB, 0x4E, 0x14
];

#[inline(always)]
fn pedersen_h() -> RistrettoPoint { RistrettoPoint::from_uniform_bytes(&H_BYTES) }

#[inline(always)]
fn scalar32(x: &[u8; 32]) -> Scalar { Scalar::from_bytes_mod_order(*x) }

/// CLASSICAL Pedersen commitment (used in ZK for speed)
/// C = r·G + v·H
/// 
/// NOTE: PQC fingerprint is NOT included in ZK commitment.
/// It's verified separately in host layer (post-ZK).
#[inline(always)]
fn pedersen_commit(value: u64, blind: &[u8; 32]) -> [u8; 32] {
    let v = Scalar::from(value);
    let r = scalar32(blind);
    (r * G + v * pedersen_h()).compress().to_bytes()
}

#[inline(always)]
fn merkle_verify(leaf: &Hash32, idx: u32, siblings: &[Hash32], root: &Hash32) -> bool {
    let mut acc = *leaf;
    let mut i = idx as usize;
    for s in siblings {
        acc = if (i & 1) == 0 { h_pair(&acc, s) } else { h_pair(s, &acc) };
        i >>= 1;
    }
    &acc == root
}

#[inline(always)]
fn make_nullifier(network_id: u32, spend_key: &[u8; 32], idx: u32, note_pt: &[u8; 32]) -> Hash32 {
    kmac32(KMAC_KEY, TAG_NF, &[&network_id.to_le_bytes(), spend_key, &idx.to_le_bytes(), note_pt])
}

/* ================== Entry ================== */

risc0_zkvm::entry!(main);
fn main() {
    let inp: PrivInput   = env::read();
    let wit: PrivWitness = env::read();

    let net_id: u32 = inp.network_id.unwrap_or(0);

    assert!(inp.ins.len() == wit.in_openings.len(), "inputs mismatch");
    assert!(inp.outs.len() == wit.out_openings.len(), "outputs mismatch");

    // Balance check (classical Pedersen in ZK)
    let mut sum_in_v = Scalar::ZERO;
    let mut sum_in_r = Scalar::ZERO;

    let mut seen_nf: Vec<Hash32> = Vec::with_capacity(inp.ins.len());
    let mut pqc_fps_in: Vec<Hash32> = Vec::with_capacity(inp.ins.len());

    for (i, ipub) in inp.ins.iter().enumerate() {
        let iopen = &wit.in_openings[i];

        assert!(ipub.proof.index == iopen.leaf_index, "leaf index mismatch");

        // Classical commitment (NO PQC fingerprint in ZK)
        let c_in = pedersen_commit(iopen.value, &iopen.blind);
        assert!(c_in == iopen.note_commit_point, "C_in mismatch vs (v,r)");

        let leaf = h_leaf(&iopen.note_commit_point);
        assert!(leaf == ipub.note_commit_hash, "leaf hash mismatch");
        assert!(merkle_verify(&leaf, ipub.proof.index, &ipub.proof.siblings, &inp.old_notes_root), "bad merkle proof");

        // Nullifier
        let nf = make_nullifier(net_id, &iopen.spend_key, iopen.leaf_index, &iopen.note_commit_point);
        assert!(nf == ipub.nullifier, "nullifier mismatch");
        assert!(!seen_nf.contains(&nf), "duplicate nullifier in tx");
        seen_nf.push(nf);

        sum_in_v += Scalar::from(iopen.value);
        sum_in_r += scalar32(&iopen.blind);
        
        // Store PQC fingerprint for host verification
        pqc_fps_in.push(ipub.pqc_fingerprint);
    }

    // Outputs
    let mut sum_out_v = Scalar::ZERO;
    let mut sum_out_r = Scalar::ZERO;

    let mut enc_hints32: Vec<Hash32> = Vec::with_capacity(inp.outs.len());
    let mut outs_points: Vec<[u8; 32]> = Vec::with_capacity(inp.outs.len());
    let mut pqc_fps_out: Vec<Hash32> = Vec::with_capacity(inp.outs.len());

    for (j, opub) in inp.outs.iter().enumerate() {
        let oopen = &wit.out_openings[j];

        // Classical commitment
        let c_out = pedersen_commit(oopen.value, &oopen.blind);
        assert!(c_out == opub.note_commit_point, "C_out mismatch");

        sum_out_v += Scalar::from(oopen.value);
        sum_out_r += scalar32(&oopen.blind);

        let hint32 = kmac32(KMAC_KEY, TAG_HINT, &[&opub.enc_hint]);
        enc_hints32.push(hint32);
        outs_points.push(opub.note_commit_point);
        pqc_fps_out.push(opub.pqc_fingerprint);
    }

    // Fee
    let fee_v = Scalar::from(wit.fee_value);
    let fee_r = scalar32(&wit.fee_blind);
    let c_fee = pedersen_commit(wit.fee_value, &wit.fee_blind);
    if inp.fee_commit != [0u8; 32] {
        assert!(c_fee == inp.fee_commit, "fee commitment mismatch");
    }

    // Balance equations (classical Pedersen)
    assert!(sum_in_v == (sum_out_v + fee_v), "value conservation failed");
    assert!(sum_in_r == (sum_out_r + fee_r), "blinding conservation failed");

    // Journal with PQC hints
    let claim = PrivClaim {
        nullifiers: inp.ins.iter().map(|x| x.nullifier).collect(),
        outputs: outs_points,
        enc_hints: enc_hints32,
        fee_commit: c_fee,
        pqc_fingerprints_in: pqc_fps_in,   // ✅ NEW
        pqc_fingerprints_out: pqc_fps_out, // ✅ NEW
    };

    env::commit(&claim);
}
