//! Cryptographic primitives module
#![forbid(unsafe_code)]

pub mod kmac;
pub mod kmac_falcon_integration;
pub mod kmac_mlkem_integration;  // ✅ NOWY: Falcon + ML-KEM (Kyber768)

pub use kmac::{kmac256_derive_key, kmac256_xof_fill, kmac256_xof};
pub use kmac_falcon_integration::{
    QuantumKeySearchCtx,
    QuantumSafeHint,
    QuantumFoundNote,
    FalconKeyManager,
    FalconError,
};

// Re-export ML-KEM types z aliasami
pub use kmac_mlkem_integration::{
    QuantumKeySearchCtx as MlkemKeySearchCtx,
    QuantumSafeHint as MlkemQuantumHint,
    QuantumFoundNote as MlkemFoundNote,
    FalconError as MlkemFalconError,
};
