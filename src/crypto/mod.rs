//! Cryptographic primitives module
#![forbid(unsafe_code)]

pub mod kmac;
pub mod kmac_falcon_integration;

pub use kmac::{kmac256_derive_key, kmac256_xof_fill, kmac256_xof};
pub use kmac_falcon_integration::{
    QuantumKeySearchCtx,
    QuantumSafeHint,
    QuantumFoundNote,
    FalconKeyManager,
    FalconError,
};
