#![forbid(unsafe_code)]

//! ZK / STARK utilities for node:
//! - stark_security: analiza parametrów STARK (BabyBear / Goldilocks / BN254)
//! - range_proof_winterfell: produkcyjny range proof 0 <= v < 2^n na Winterfell 0.13

pub mod stark_security;

#[cfg(feature = "winterfell_v2")]
pub mod range_proof_winterfell;
