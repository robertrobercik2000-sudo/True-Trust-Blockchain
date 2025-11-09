//! KMAC bridge for consensus module
//!
//! This module provides KMAC256 hashing compatible with consensus requirements.
//! It's a thin wrapper around the crypto::kmac module.

#![forbid(unsafe_code)]

use crate::crypto::kmac::kmac256_xof_fill;

/// KMAC256 hash function for consensus (domain, concatenated inputs)
///
/// # Parameters
/// - `domain`: Domain separation label
/// - `inputs`: Slice of byte slices to concatenate and hash
///
/// # Returns
/// 32-byte hash output
#[inline]
pub fn kmac256_hash(domain: &[u8], inputs: &[&[u8]]) -> [u8; 32] {
    let mut combined = Vec::new();
    for input in inputs {
        combined.extend_from_slice(input);
    }
    
    let mut output = [0u8; 32];
    kmac256_xof_fill(&combined, domain, b"", &mut output);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kmac256_hash() {
        let result = kmac256_hash(b"TEST", &[b"hello", b"world"]);
        assert_ne!(result, [0u8; 32]);
        
        // Same inputs -> same output
        let result2 = kmac256_hash(b"TEST", &[b"hello", b"world"]);
        assert_eq!(result, result2);
        
        // Different order -> different output
        let result3 = kmac256_hash(b"TEST", &[b"world", b"hello"]);
        assert_ne!(result, result3);
    }
}
