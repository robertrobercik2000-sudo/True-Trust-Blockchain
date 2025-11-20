#![forbid(unsafe_code)]

//! KMAC256 hash functions for consensus and cryptographic operations
//! Supports both SHAKE256 (XOF) and SHA3-512 (higher security) backends

use sha3::{
    Digest, Sha3_512, Shake256,
    digest::{Update, ExtendableOutput, XofReader},
};

/// KMAC256 hash using SHA3-512 (32 bytes output, truncated from 64)
/// Higher security level: 256-bit vs SHAKE256's 128-bit
/// Uses a fixed key for consensus operations (domain separation via label)
pub fn kmac256_hash(label: &[u8], inputs: &[&[u8]]) -> [u8; 32] {
    const CONSENSUS_KEY: &[u8] = b"TT-CONSENSUS-KMAC256-v2";
    
    let mut hasher = Sha3_512::new();
    Digest::update(&mut hasher, b"KMAC256-HASH-v2");
    Digest::update(&mut hasher, &(CONSENSUS_KEY.len() as u64).to_le_bytes());
    Digest::update(&mut hasher, CONSENSUS_KEY);
    Digest::update(&mut hasher, &(label.len() as u64).to_le_bytes());
    Digest::update(&mut hasher, label);
    for input in inputs {
        Digest::update(&mut hasher, &(input.len() as u64).to_le_bytes());
        Digest::update(&mut hasher, input);
    }
    
    let result = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result[..32]);
    out
}

/// Legacy KMAC256 hash using SHAKE256 (backward compatibility)
pub fn kmac256_hash_v1(label: &[u8], inputs: &[&[u8]]) -> [u8; 32] {
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

/// KMAC256 key derivation function (32 bytes output)
pub fn kmac256_derive_key(key: &[u8], label: &[u8], context: &[u8]) -> [u8; 32] {
    let mut hasher = Shake256::default();
    Update::update(&mut hasher, b"KMAC256-DERIVE-v1");
    Update::update(&mut hasher, &(key.len() as u64).to_le_bytes());
    Update::update(&mut hasher, key);
    Update::update(&mut hasher, &(label.len() as u64).to_le_bytes());
    Update::update(&mut hasher, label);
    Update::update(&mut hasher, &(context.len() as u64).to_le_bytes());
    Update::update(&mut hasher, context);
    
    let mut reader = hasher.finalize_xof();
    let mut out = [0u8; 32];
    XofReader::read(&mut reader, &mut out);
    out
}

/// KMAC256 message authentication code (32 bytes output)
pub fn kmac256_tag(key: &[u8], label: &[u8], data: &[u8]) -> [u8; 32] {
    let mut hasher = Shake256::default();
    Update::update(&mut hasher, b"KMAC256-MAC-v1");
    Update::update(&mut hasher, &(key.len() as u64).to_le_bytes());
    Update::update(&mut hasher, key);
    Update::update(&mut hasher, &(label.len() as u64).to_le_bytes());
    Update::update(&mut hasher, label);
    Update::update(&mut hasher, &(data.len() as u64).to_le_bytes());
    Update::update(&mut hasher, data);
    
    let mut reader = hasher.finalize_xof();
    let mut out = [0u8; 32];
    XofReader::read(&mut reader, &mut out);
    out
}

/// KMAC256 XOF - variable length output
pub fn kmac256_xof(key: &[u8], label: &[u8], context: &[u8], out_len: usize) -> Vec<u8> {
    let mut hasher = Shake256::default();
    Update::update(&mut hasher, b"KMAC256-XOF-v1");
    Update::update(&mut hasher, &(key.len() as u64).to_le_bytes());
    Update::update(&mut hasher, key);
    Update::update(&mut hasher, &(label.len() as u64).to_le_bytes());
    Update::update(&mut hasher, label);
    Update::update(&mut hasher, &(context.len() as u64).to_le_bytes());
    Update::update(&mut hasher, context);
    
    let mut reader = hasher.finalize_xof();
    let mut out = vec![0u8; out_len];
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

    #[test]
    fn test_sha3_512_vs_shake256() {
        let label = b"TEST";
        let input = b"data";
        
        let h_sha3 = kmac256_hash(label, &[input]);
        let h_shake = kmac256_hash_v1(label, &[input]);
        
        assert_ne!(h_sha3, h_shake);
        assert_eq!(h_sha3.len(), 32);
        assert_eq!(h_shake.len(), 32);
    }
}
