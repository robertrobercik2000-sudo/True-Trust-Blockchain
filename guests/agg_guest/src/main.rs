#![no_std]
#![no_main]
extern crate alloc;

use alloc::vec::Vec;
use risc0_zkvm::guest::env;
use risc0_zkvm::serde::from_slice;
use serde::{Deserialize, Serialize};
use tiny_keccak::{Hasher, Kmac};

type Hash32 = [u8; 32];

/* ================== Konfiguracja / limity ================== */

const EMPTY_ROOT: Hash32 = [0u8; 32];

const MAX_CHILDREN: usize = 64;
const MAX_CHILD_WORDS: usize = 1 << 18;
const MAX_OUTS_TOTAL: usize = 1 << 16;
const MAX_HINTS_TOTAL: usize = 1 << 16;

const KMAC_KEY: &[u8] = b"agg-priv:v1";
const TAG_LEAF: &[u8] = b"LEAF.v1";
const TAG_NODE: &[u8] = b"NODE.v1";
const TAG_CLAIM: &[u8] = b"CLAIM.v1";

/* ================== I/O (with PQC support) ================== */

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivClaim {
    pub nullifiers: Vec<Hash32>,
    pub outputs: Vec<[u8; 32]>,
    pub enc_hints: Vec<[u8; 32]>,
    pub fee_commit: [u8; 32],
    // ✅ NEW: PQC fingerprints from child
    pub pqc_fingerprints_in: Vec<Hash32>,
    pub pqc_fingerprints_out: Vec<Hash32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AggPrivInput {
    pub old_notes_root: Hash32,
    pub old_notes_count: u64,
    pub old_frontier: Vec<Hash32>,
    pub child_method_id: [u32; 8],
    pub claim_receipts_words: Vec<Vec<u32>>,
    pub claim_journals_words: Vec<Vec<u32>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AggPrivJournal {
    pub new_notes_root: Hash32,
    pub new_notes_count: u64,
    pub new_frontier: Vec<Hash32>,
    pub nullifiers: Vec<Hash32>,
    pub enc_hints: Vec<[u8; 32]>,
    pub outputs_count: u32,
    pub child_claim_tags: Vec<[u8; 32]>,
    // ✅ NEW: Aggregated PQC fingerprints
    pub pqc_fingerprints_in: Vec<Hash32>,
    pub pqc_fingerprints_out: Vec<Hash32>,
}

/* ================== KMAC / Merkle ================== */

#[inline(always)]
fn kmac32(key: &[u8], custom: &[u8], parts: &[&[u8]]) -> Hash32 {
    let mut km = Kmac::v256(key, custom);
    for p in parts { km.update(p); }
    let mut o = [0u8; 32];
    km.finalize(&mut o);
    o
}

#[inline(always)]
fn kmac32_words(key: &[u8], custom: &[u8], words: &[u32]) -> Hash32 {
    let mut km = Kmac::v256(key, custom);
    for w in words { km.update(&w.to_le_bytes()); }
    let mut o = [0u8; 32];
    km.finalize(&mut o);
    o
}

#[inline(always)]
fn h_pair(l: &Hash32, r: &Hash32) -> Hash32 { kmac32(KMAC_KEY, TAG_NODE, &[l, r]) }

#[inline(always)]
fn h_leaf(pt: &[u8; 32]) -> Hash32 { kmac32(KMAC_KEY, TAG_LEAF, &[pt]) }

fn root_from_frontier(frontier: &[Hash32], count: u64) -> Hash32 {
    if count == 0 { return EMPTY_ROOT; }

    let mut it = frontier.iter();
    let mut acc: Option<Hash32> = None;
    let mut n = count;

    while n > 0 {
        if (n & 1) == 1 {
            let f = it.next().expect("frontier too short");
            acc = Some(match acc { None => *f, Some(ref a) => h_pair(f, a) });
        }
        n >>= 1;
    }

    assert!(it.next().is_none(), "frontier too long");
    acc.expect("nonzero count but empty frontier")
}

fn append_leaf(frontier: &mut Vec<Hash32>, count: &mut u64, mut cur: Hash32) {
    let mut lvl = 0usize;
    let mut c = *count;

    while (c & 1) == 1 {
        let left = frontier[lvl];
        cur = h_pair(&left, &cur);
        c >>= 1;
        lvl += 1;
    }

    if frontier.len() <= lvl { frontier.resize(lvl + 1, [0u8; 32]); }
    frontier[lvl] = cur;
    *count += 1;
}

/* ================== Entry ================== */

risc0_zkvm::entry!(main);
fn main() {
    let inp: AggPrivInput = env::read();

    assert!(inp.claim_receipts_words.len() <= MAX_CHILDREN, "too many children");
    assert!(inp.claim_journals_words.len() == inp.claim_receipts_words.len(), "receipts/journals length mismatch");

    let calc_old = root_from_frontier(&inp.old_frontier, inp.old_notes_count);
    assert!(calc_old == inp.old_notes_root, "old frontier/count mismatch");

    let mut all_nfs: Vec<Hash32> = Vec::new();
    let mut all_outs: Vec<[u8; 32]> = Vec::new();
    let mut all_hints: Vec<[u8; 32]> = Vec::new();
    let mut child_tags: Vec<[u8; 32]> = Vec::with_capacity(inp.claim_journals_words.len());
    
    // ✅ NEW: Aggregate PQC fingerprints
    let mut all_pqc_fps_in: Vec<Hash32> = Vec::new();
    let mut all_pqc_fps_out: Vec<Hash32> = Vec::new();

    for (i, rb) in inp.claim_receipts_words.iter().enumerate() {
        assert!(rb.len() <= MAX_CHILD_WORDS, "child receipt too big");
        let jw: &[u32] = &inp.claim_journals_words[i];
        assert!(jw.len() <= MAX_CHILD_WORDS, "child journal too big");

        env::verify(inp.child_method_id, rb).expect("child receipt verify");

        let claim_tag = kmac32_words(KMAC_KEY, TAG_CLAIM, jw);
        child_tags.push(claim_tag);

        let claim: PrivClaim = from_slice(jw).expect("decode child journal");
        all_nfs.extend_from_slice(&claim.nullifiers);
        all_outs.extend_from_slice(&claim.outputs);
        all_hints.extend_from_slice(&claim.enc_hints);
        
        // ✅ NEW: Aggregate PQC fingerprints
        all_pqc_fps_in.extend_from_slice(&claim.pqc_fingerprints_in);
        all_pqc_fps_out.extend_from_slice(&claim.pqc_fingerprints_out);
    }

    assert!(all_outs.len() <= MAX_OUTS_TOTAL, "too many outputs");
    assert!(all_hints.len() <= MAX_HINTS_TOTAL, "too many hints");

    let mut cnt = inp.old_notes_count;
    let mut fr = inp.old_frontier;

    for c in all_outs.iter() { append_leaf(&mut fr, &mut cnt, h_leaf(c)); }
    let new_root = root_from_frontier(&fr, cnt);

    all_nfs.sort_unstable();
    for w in all_nfs.windows(2) { assert!(w[0] != w[1], "duplicate NF in block"); }

    assert!(all_outs.len() <= u32::MAX as usize, "outputs_count overflow u32");

    let out = AggPrivJournal {
        new_notes_root: new_root,
        new_notes_count: cnt,
        new_frontier: fr,
        nullifiers: all_nfs,
        enc_hints: all_hints,
        outputs_count: all_outs.len() as u32,
        child_claim_tags: child_tags,
        pqc_fingerprints_in: all_pqc_fps_in,   // ✅ NEW
        pqc_fingerprints_out: all_pqc_fps_out, // ✅ NEW
    };

    env::commit(&out);
}
