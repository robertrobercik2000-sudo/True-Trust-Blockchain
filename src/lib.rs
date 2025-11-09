//! Quantum Falcon Wallet - Advanced Post-Quantum Cryptography
//! 
//! KMAC256 + Falcon512 hybrid keysearch implementation
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod crypto;
pub mod keysearch;

// Re-export main types
pub use crypto::{
    QuantumKeySearchCtx,
    QuantumSafeHint,
    QuantumFoundNote,
    FalconKeyManager,
    FalconError,
    kmac256_derive_key,
};

pub use keysearch::{
    HintPayloadV1,
    DecodedHint,
    KeySearchCtx,
    AadMode,
};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Check quantum support
pub fn has_quantum_support() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_version() {
        assert!(!VERSION.is_empty());
        println!("Library version: {}", VERSION);
    }

    #[test]
    fn test_quantum_available() {
        assert!(has_quantum_support());
    }
}
