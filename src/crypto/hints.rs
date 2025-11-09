//! Unified hint scanner trait for privacy layers
//! 
//! Provides a common interface for both legacy (X25519-only) and quantum-safe
//! (PQC) hint scanning implementations.

#![forbid(unsafe_code)]

use crate::keysearch::{HintPayloadV1, DecodedHint};

/// Public keys for hint encryption
pub trait PublicKeys {
    /// Get X25519 public key (always required)
    fn x25519_public(&self) -> &[u8; 32];
    
    /// Get ML-KEM public key (optional, for PQC)
    fn mlkem_public(&self) -> Option<&[u8]> {
        None
    }
    
    /// Get Falcon public key (optional, for PQC)
    fn falcon_public(&self) -> Option<&[u8]> {
        None
    }
}

/// Result of hint verification
#[derive(Debug, Clone)]
pub struct ScanResult {
    /// Decoded hint data
    pub decoded: DecodedHint,
    
    /// Whether timestamp/epoch validation passed
    pub fresh: bool,
    
    /// Hint type (for telemetry)
    pub hint_type: HintType,
}

/// Hint encryption type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintType {
    /// Legacy X25519 + AES-GCM
    Legacy,
    
    /// Quantum-safe ML-KEM + X25519 + Falcon
    QuantumSafe,
}

/// Unified hint scanner trait
/// 
/// Implementations:
/// - `KeySearchCtx` (legacy X25519)
/// - `QuantumKeySearchCtx` (PQC)
pub trait HintScanner {
    /// Associated hint type
    type Hint;
    
    /// Error type
    type Error: std::error::Error;
    
    /// Build encrypted hint for recipient
    /// 
    /// # Parameters
    /// - `recipient`: Recipient's public keys
    /// - `c_out`: Output commitment (binding parameter)
    /// - `payload`: Hint data (r_blind, value, memo)
    /// 
    /// # Returns
    /// Encrypted hint or error
    fn build_hint(
        &self,
        recipient: &dyn PublicKeys,
        c_out: &[u8; 32],
        payload: &HintPayloadV1,
    ) -> Result<Self::Hint, Self::Error>;
    
    /// Scan and decrypt hint
    /// 
    /// # Parameters
    /// - `hint`: Encrypted hint
    /// - `c_out`: Output commitment (must match)
    /// 
    /// # Returns
    /// `Some(ScanResult)` if hint is for us and valid, `None` otherwise
    fn scan_hint(
        &self,
        hint: &Self::Hint,
        c_out: &[u8; 32],
    ) -> Option<ScanResult>;
    
    /// Fast pre-filter (Bloom filter optimization)
    /// 
    /// Returns 16-byte fingerprint for Bloom filter insertion/lookup.
    /// Allows O(1) rejection of hints not for this wallet.
    fn hint_fingerprint(hint: &Self::Hint, c_out: &[u8; 32]) -> [u8; 16];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hint_type_size() {
        // HintType should be 1 byte
        assert_eq!(std::mem::size_of::<HintType>(), 1);
    }
}
