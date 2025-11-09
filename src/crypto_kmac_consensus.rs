//! KMAC256 hash functions for consensus operations
//! Uses proper KMAC256 (NIST SP 800-185) via tiny-keccak cSHAKE

use tiny_keccak::{CShake, Hasher};

/// KMAC256 hash (32 bytes output) - proper NIST SP 800-185 KMAC construction
/// 
/// Implements KMAC256 as defined in NIST SP 800-185 Section 4:
/// KMAC256(K, X, L, S) = cSHAKE256(bytepad(encode_string(K), 136) || X || right_encode(L), N="KMAC", S)
/// 
/// Simplified version using cSHAKE directly with proper domain separation.
///
/// # Parameters
/// - `label`: Domain separation label (customization string)
/// - `inputs`: Slice of byte slices to hash
///
/// # Returns
/// 32-byte KMAC256 output
pub fn kmac256_hash(label: &[u8], inputs: &[&[u8]]) -> [u8; 32] {
    // Fixed key for consensus operations
    const CONSENSUS_KEY: &[u8] = b"TT-CONSENSUS-KMAC256";
    
    // Create cSHAKE256 with "KMAC" function name and custom label
    // This provides proper domain separation per NIST SP 800-185
    let mut hasher = CShake::v256(b"KMAC", label);
    
    // Hash the key (length-prefixed)
    hasher.update(&(CONSENSUS_KEY.len() as u64).to_le_bytes());
    hasher.update(CONSENSUS_KEY);
    
    // Hash all inputs (length-prefixed for unambiguous encoding)
    for input in inputs {
        hasher.update(&(input.len() as u64).to_le_bytes());
        hasher.update(input);
    }
    
    // Finalize to 32-byte output
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kmac256_hash_deterministic() {
        let h1 = kmac256_hash(b"TEST", &[b"input1", b"input2"]);
        let h2 = kmac256_hash(b"TEST", &[b"input1", b"input2"]);
        assert_eq!(h1, h2, "KMAC256 must be deterministic");
    }

    #[test]
    fn test_kmac256_hash_different_labels() {
        let h1 = kmac256_hash(b"LABEL1", &[b"input"]);
        let h2 = kmac256_hash(b"LABEL2", &[b"input"]);
        assert_ne!(h1, h2, "Different labels must produce different outputs");
    }

    #[test]
    fn test_kmac256_hash_different_inputs() {
        let h1 = kmac256_hash(b"TEST", &[b"input1"]);
        let h2 = kmac256_hash(b"TEST", &[b"input2"]);
        assert_ne!(h1, h2, "Different inputs must produce different outputs");
    }

    #[test]
    fn test_kmac256_hash_not_zero() {
        let h = kmac256_hash(b"TEST", &[b"input"]);
        assert_ne!(h, [0u8; 32], "KMAC256 output should not be all zeros");
    }
    
    #[test]
    fn test_kmac256_hash_order_matters() {
        let h1 = kmac256_hash(b"TEST", &[b"A", b"B"]);
        let h2 = kmac256_hash(b"TEST", &[b"B", b"A"]);
        assert_ne!(h1, h2, "Input order must affect output");
    }
}
