//! KMAC256 hash functions for consensus operations
//! Uses SHAKE256 (SHA3 XOF) as the underlying primitive

use sha3::{
    digest::{ExtendableOutput, Update, XofReader},
    Shake256,
};

/// KMAC256 hash (32 bytes output) - deterministic hash function
/// Uses a fixed key for consensus operations (domain separation via label)
pub fn kmac256_hash(label: &[u8], inputs: &[&[u8]]) -> [u8; 32] {
    // Fixed key for consensus operations (domain separation via label)
    const CONSENSUS_KEY: &[u8] = b"TT-CONSENSUS-KMAC256";

    let mut hasher = Shake256::default();
    Update::update(&mut hasher, b"KMAC256-HASH-v1");
    Update::update(&mut hasher, &(CONSENSUS_KEY.len() as u64).to_le_bytes());
    Update::update(&mut hasher, CONSENSUS_KEY);
    Update::update(&mut hasher, &(label.len() as u64).to_le_bytes());
    Update::update(&mut hasher, label);
    for input in inputs {
        Update::update(&mut hasher, &(input.len() as u64).to_le_bytes());
        Update::update(&mut hasher, input);
    }

    let mut reader = hasher.finalize_xof();
    let mut out = [0u8; 32];
    XofReader::read(&mut reader, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kmac256_hash_deterministic() {
        let h1 = kmac256_hash(b"TEST", &[b"input1", b"input2"]);
        let h2 = kmac256_hash(b"TEST", &[b"input1", b"input2"]);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_kmac256_hash_different_labels() {
        let h1 = kmac256_hash(b"LABEL1", &[b"input"]);
        let h2 = kmac256_hash(b"LABEL2", &[b"input"]);
        assert_ne!(h1, h2);
    }
}
