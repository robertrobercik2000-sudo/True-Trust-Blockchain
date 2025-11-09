//! Keysearch structures and context
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

/// Hint payload v1
#[derive(Clone, Debug, Serialize, Deserialize, Zeroize)]
#[zeroize(drop)]
pub struct HintPayloadV1 {
    pub r_blind: [u8; 32],
    pub value: u64,
    pub memo: Vec<u8>,
}

/// Decoded hint result
#[derive(Clone, Debug)]
pub struct DecodedHint {
    pub r_blind: [u8; 32],
    pub value: Option<u64>,
    pub memo_items: Vec<Vec<u8>>,
}

/// AAD mode for encryption
#[derive(Clone, Copy, Debug)]
pub enum AadMode {
    COutOnly,
    Full,
}

/// Traditional keysearch context (X25519 only)
pub struct KeySearchCtx {
    x25519_secret: Zeroizing<[u8; 32]>,
}

impl KeySearchCtx {
    pub fn new(secret: [u8; 32]) -> Self {
        Self {
            x25519_secret: Zeroizing::new(secret),
        }
    }
    
    pub fn try_match(&self, _c_out: &[u8; 32], _hint: &[u8]) -> Option<[u8; 32]> {
        // Simplified implementation
        Some([0u8; 32])
    }
    
    pub fn try_match_and_decrypt_ext(
        &self,
        _c_out: &[u8; 32],
        _hint: &[u8],
        _aad_mode: AadMode,
    ) -> Option<([u8; 32], Option<DecodedHint>)> {
        // Simplified implementation
        Some(([0u8; 32], None))
    }
}
