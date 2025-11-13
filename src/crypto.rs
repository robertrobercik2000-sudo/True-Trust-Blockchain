//! Cryptographic primitives for consensus and wallet operations

use sha3::{Digest, Sha3_512, Shake256, digest::{Update, ExtendableOutput, XofReader}};

/// KMAC256 hash using SHA3-512 (256-bit security)
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

/// Legacy KMAC256 using SHAKE256 (backward compatibility)
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

/// KMAC256 key derivation
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha3_512_deterministic() {
        let h1 = kmac256_hash(b"TEST", &[b"data"]);
        let h2 = kmac256_hash(b"TEST", &[b"data"]);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_sha3_vs_shake() {
        let h1 = kmac256_hash(b"TEST", &[b"data"]);
        let h2 = kmac256_hash_v1(b"TEST", &[b"data"]);
        assert_ne!(h1, h2);
    }
}
