#![forbid(unsafe_code)]

//! Zero-Knowledge Proofs Module
//!
//! Contains:
//! - RISC0 zkVM integration (simplified for TRUE_TRUST)
//! - Winterfell STARK range proofs (production-grade)

pub mod range_stark_winterfell; // Winterfell 0.9 stub (API mismatch)
pub mod range_stark_winterfell_v2; // Winterfell 0.13 (ready for Rust 1.87+)

use serde::{Deserialize, Serialize};
use crate::core::Hash32;

// ===== Data structures =====

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct InPublic {
    pub nf: Hash32,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct OutPublic {
    pub eph_pub: [u8; 32],
    pub C_out: [u8; 32],
    pub filter_tag16: u16,
    pub enc_hints: Vec<u8>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct InOpen {
    pub amt: u64,
    pub blind: [u8; 32],
    pub pk: Vec<u8>,
    pub idx: u64,
    pub path: Vec<Hash32>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct OutOpen {
    pub amt: u64,
    pub blind: [u8; 32],
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct OutBp {
    pub proof_bytes: Vec<u8>,
    pub C_out: [u8; 32],
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PrivInput {
    pub state_root: Hash32,
    pub H_pedersen: [u8; 32],
    pub ins_pub: Vec<InPublic>,
    pub outs_pub: Vec<OutPublic>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PrivWitness {
    pub ins_open: Vec<InOpen>,
    pub outs_open: Vec<OutOpen>,
    pub outs_bp: Vec<OutBp>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PrivClaim {
    pub state_root: Hash32,
    pub sum_in_amt: u64,
    pub sum_out_amt: u64,
    pub ins_nf: Vec<Hash32>,
    pub outs_data: Vec<OutPublic>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AggPrivInput {
    pub state_root: Hash32,
    pub receipts_ser: Vec<Vec<u8>>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AggPrivJournal {
    pub state_root: Hash32,
    pub sum_in_amt: u64,
    pub sum_out_amt: u64,
    pub ins_nf: Vec<Hash32>,
    pub outs_data: Vec<OutPublic>,
}

// ===== Prover/Verifier functions (stubs) =====

/// Child proof for single private tx
#[cfg(feature = "risc0-prover")]
pub fn prove_priv_claim(
    input: &PrivInput,
    witness: &PrivWitness
) -> anyhow::Result<(PrivClaim, Vec<u8>)> {
    // TODO: integrate risc0_zkvm::Prover + methods_priv::PRIV_ELF
    unimplemented!("prove_priv_claim requires risc0-prover feature + ELF")
}

#[cfg(not(feature = "risc0-prover"))]
pub fn prove_priv_claim(
    _input: &PrivInput,
    _witness: &PrivWitness
) -> anyhow::Result<(PrivClaim, Vec<u8>)> {
    Err(anyhow::anyhow!("risc0-prover feature disabled"))
}

/// Verify child proof
pub fn verify_priv_receipt(
    receipt_bytes: &[u8],
    expected_state_root: &Hash32
) -> anyhow::Result<PrivClaim> {
    // TODO: integrate risc0_zkvm::Receipt::verify + check journal
    let _receipt = receipt_bytes; // placeholder
    let _root = expected_state_root;
    unimplemented!("verify_priv_receipt requires risc0 zkVM")
}

/// Aggregation proof
#[cfg(feature = "risc0-prover")]
pub fn prove_agg_priv_with_receipts(
    receipts_ser: Vec<Vec<u8>>,
    state_root: Hash32
) -> anyhow::Result<(AggPrivJournal, Vec<u8>)> {
    // TODO: integrate risc0_zkvm::Prover + methods_agg_priv::AGG_PRIV_ELF
    let _ = receipts_ser;
    let _ = state_root;
    unimplemented!("prove_agg_priv_with_receipts requires risc0-prover + ELF")
}

#[cfg(not(feature = "risc0-prover"))]
pub fn prove_agg_priv_with_receipts(
    _receipts_ser: Vec<Vec<u8>>,
    _state_root: Hash32
) -> anyhow::Result<(AggPrivJournal, Vec<u8>)> {
    Err(anyhow::anyhow!("risc0-prover feature disabled"))
}

/// Verify aggregation proof
pub fn verify_agg_receipt(
    receipt_bytes: &[u8],
    expected_state_root: &Hash32
) -> anyhow::Result<AggPrivJournal> {
    let _ = receipt_bytes;
    let _ = expected_state_root;
    unimplemented!("verify_agg_receipt requires risc0 zkVM")
}

// ===== Helpers =====

pub fn bytes_to_words(bytes: &[u8]) -> Vec<u32> {
    bytes.chunks(4).map(|ch| {
        let mut arr = [0u8; 4];
        arr[..ch.len()].copy_from_slice(ch);
        u32::from_le_bytes(arr)
    }).collect()
}
