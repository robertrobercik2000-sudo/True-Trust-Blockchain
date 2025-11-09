//! Cryptographic primitives module
//! 
//! **IMPORTANT SECURITY NOTE:**
//! - Falcon used ONLY for signatures (never KEX!)
//! - ML-KEM (Kyber768) for key encapsulation
//! - XChaCha20-Poly1305 for AEAD
//! - Transcript binding prevents mix-and-match attacks
#![forbid(unsafe_code)]

pub mod kmac;
pub mod kmac_drbg;          // ✅ NEW: KMAC-based DRBG (no_std, RngCore, CryptoRng)
pub mod kmac_falcon_integration;
pub mod kmac_mlkem_integration;
pub mod hint_transcript;    // ✅ NEW: Transcript + AEAD helpers
pub mod quantum_hint_v2;    // ✅ NEW: CORRECTED hint implementation

#[cfg(feature = "seeded_falcon")]
pub mod seeded;             // ✅ NEW: Deterministic Falcon via KMAC-DRBG + FFI

pub use kmac::{kmac256_derive_key, kmac256_xof_fill, kmac256_xof};

// ✅ Corrected Falcon implementation (Falcon=sig, ML-KEM=KEX, XChaCha=AEAD)
pub use kmac_falcon_integration::{
    QuantumKeySearchCtx,  // Main API
    QuantumSafeHint,      // Main API
    QuantumFoundNote,
    FalconKeyManager,
    FalconError,
    MlkemPublicKey,
    MlkemSecretKey,
    hint_fingerprint16,  // ✅ NEW: Bloom filter integration
    DEFAULT_MAX_SKEW_SECS,
    DEFAULT_ACCEPT_PREV_EPOCH,
};

// ML-KEM module (transitional)
pub use kmac_mlkem_integration::{
    QuantumKeySearchCtx as MlkemKeySearchCtx,
    QuantumSafeHint as MlkemQuantumHint,
    QuantumFoundNote as MlkemFoundNote,
    FalconError as MlkemFalconError,
};

// ✅ RECOMMENDED: Use v2 API (correct Falcon usage)
// Re-exported from quantum_hint_v2 after implementation
