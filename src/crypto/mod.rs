//! Cryptographic Utilities for TRUE_TRUST
//!
//! - KMAC: KMAC256 primitives (SHA3-based MAC + XOF)
//! - KMAC-DRBG: Deterministic random bit generator using KMAC256
//! - Seeded Falcon: Deterministic Falcon-512 key generation and signing

#![forbid(unsafe_code)]

pub mod kmac;
pub mod kmac_drbg;

#[cfg(feature = "seeded_falcon")]
pub mod seeded;
