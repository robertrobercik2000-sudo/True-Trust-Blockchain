#![forbid(unsafe_code)]

//! Winterfell STARK Range Proofs - STUB
//!
//! **Status:** NOT IMPLEMENTED - API/version incompatibility
//!
//! **Problem:**
//! - Winterfell 0.9: API mismatch (missing GkrProof, wrong signatures)
//! - Winterfell 0.13+: Requires Rust 1.87+, we have 1.82
//!
//! **Solution:** Use our Goldilocks STARK (src/stark_goldilocks.rs) instead.
//! It's working, 64-bit secure, just unoptimized.
//!
//! **Future:** Integrate Winterfell when:
//! 1. Rust upgraded to 1.87+, OR
//! 2. Winterfell API stabilizes, OR
//! 3. We port code to match current Winterfell API

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct ProofOptions;

/// Stub: prove range bound
///
/// TODO: Integrate Winterfell STARK when Rust 1.87+ available
pub fn prove_range_bound(_value: u64, _commitment: [u8; 32], _opts: Option<ProofOptions>) -> Vec<u8> {
    unimplemented!(
        "Winterfell STARK requires Rust 1.87+ (we have 1.82). \
         Use stark_goldilocks::prove_range instead."
    )
}

/// Stub: verify range bound with commitment
///
/// TODO: Integrate Winterfell STARK when Rust 1.87+ available
pub fn verify_range_bound_with_commitment(_bytes: &[u8], _expected_commitment: [u8; 32]) -> bool {
    unimplemented!(
        "Winterfell STARK requires Rust 1.87+ (we have 1.82). \
         Use stark_goldilocks::verify instead."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "not implemented")]
    fn test_stub_panics() {
        prove_range_bound(42, [0u8; 32], None);
    }
}
