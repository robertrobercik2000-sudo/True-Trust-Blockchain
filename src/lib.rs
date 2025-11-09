//! Quantum-Safe Cryptocurrency Wallet Library
//! 
//! Post-quantum secure cryptographic operations using Falcon512 + X25519
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

/// Check if post-quantum support is available
pub fn has_quantum_support() -> bool {
    true // Falcon is always available with pqcrypto-falcon
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
        assert!(has_quantum_support());
        println!("Quantum support: enabled");
    }
    
    #[test]
    fn test_unified_keysearch_basic() {
        let seed = [0x55u8; 32];
        let unified = UnifiedKeySearch::new(seed);
        
        assert!(unified.has_quantum_support());
        
        let keys = unified.get_public_keys();
        assert_eq!(keys.x25519_pk.len(), 32);
        assert!(keys.falcon_pk.is_some());
        
        if let Some(ref falcon_pk) = keys.falcon_pk {
            assert_eq!(falcon_pk.len(), 897); // Falcon512 public key size
        }
    }
}
