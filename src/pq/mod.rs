#![forbid(unsafe_code)]

//! Post-Quantum (PQ) Cryptography Module
//!
//! Contains PQ-secure transaction primitives using:
//! - Falcon512 (signatures)
//! - Kyber768 (KEM)
//! - STARK (range proofs via Winterfell)

pub mod tx_stark;
