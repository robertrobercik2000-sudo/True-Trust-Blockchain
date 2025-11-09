//! Cryptographic primitives and quantum-safe extensions
#![forbid(unsafe_code)]

pub mod kmac;
pub mod falcon_integration;
pub mod keysearch_quantum;

// Re-exports
pub use falcon_integration::{
    QuantumKeySearchCtx,
    QuantumSafeHint,
    HintPayload,
    FalconKeyManager,
    FalconError,
};

pub use keysearch_quantum::{
    UnifiedKeySearch,
    PublicKeys,
    SmartHint,
    FoundNote,
    BuildError,
};

pub use kmac::{
    kmac256_derive_key,
    kmac256_xof,
    kmac256_xof_fill,
    kmac256_tag,
};
