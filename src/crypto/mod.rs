//! Cryptographic primitives module
//! 
//! **IMPORTANT SECURITY NOTE:**
//! - Falcon used ONLY for signatures (never KEX!)
//! - ML-KEM (Kyber768) for key encapsulation
//! - XChaCha20-Poly1305 for AEAD
//! - Transcript binding prevents mix-and-match attacks
#![forbid(unsafe_code)]

pub mod kmac;
pub mod kmac_drbg;                    // KMAC-based DRBG (no_std, RngCore, CryptoRng)
pub mod falcon_ops;                   // Unified Falcon operations (trait-based)
pub mod kmac_falcon_integration;      // Main quantum-safe implementation

#[cfg(feature = "seeded_falcon")]
pub mod seeded;                       // Deterministic Falcon via KMAC-DRBG + FFI

pub use kmac::{kmac256_derive_key, kmac256_xof_fill, kmac256_xof};

// Main quantum-safe API (Falcon=signatures, ML-KEM=KEX, XChaCha=AEAD)
pub use kmac_falcon_integration::{
    QuantumKeySearchCtx,
    QuantumSafeHint,
    QuantumFoundNote,
    FalconKeyManager,
    FalconError,
    MlkemPublicKey,
    MlkemSecretKey,
    hint_fingerprint16,
    DEFAULT_MAX_SKEW_SECS,
    DEFAULT_ACCEPT_PREV_EPOCH,
};
