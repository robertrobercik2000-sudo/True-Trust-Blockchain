//! Quantum-Safe Cryptocurrency Wallet Library
//! 
//! This library provides post-quantum secure cryptographic operations
//! using Falcon512 signatures combined with traditional X25519 key exchange
//! for hybrid security.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod crypto;

// Re-export main types
pub use crypto::{
    UnifiedKeySearch,
    QuantumKeySearchCtx,
    PublicKeys,
    SmartHint,
    FoundNote,
    HintPayload,
    QuantumSafeHint,
    FalconError,
    BuildError,
};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Check if post-quantum support is compiled in
pub fn has_quantum_support() -> bool {
    // This will be true if pqcrypto-falcon is available
    cfg!(feature = "default")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
        println!("Library version: {}", VERSION);
    }

    #[test]
    fn test_quantum_support() {
        println!("Quantum support: {}", has_quantum_support());
    }
}
