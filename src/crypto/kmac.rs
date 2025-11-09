//! KMAC256 cryptographic functions for wallet operations
//! Uses SHAKE256 (SHA3 XOF) as the underlying primitive

use sha3::{Shake256, digest::{Update, ExtendableOutput, XofReader}};

/// KMAC256 key derivation function
/// Derives a 32-byte key from password, label, and salt
pub fn kmac256_derive_key(password: &[u8], label: &[u8], salt: &[u8]) -> [u8; 32] {
    let mut hasher = Shake256::default();
    Update::update(&mut hasher, b"KMAC256-KDF-v1");
    Update::update(&mut hasher, &(password.len() as u64).to_le_bytes());
    Update::update(&mut hasher, password);
    Update::update(&mut hasher, &(label.len() as u64).to_le_bytes());
    Update::update(&mut hasher, label);
    Update::update(&mut hasher, &(salt.len() as u64).to_le_bytes());
    Update::update(&mut hasher, salt);
    
    let mut reader = hasher.finalize_xof();
    let mut out = [0u8; 32];
    XofReader::read(&mut reader, &mut out);
    out
}

/// KMAC256 XOF (extendable output function)
/// Generates a variable-length output stream
pub fn kmac256_xof(input: &[u8], label: &[u8], salt: &[u8], output_len: usize) -> Vec<u8> {
    let mut hasher = Shake256::default();
    Update::update(&mut hasher, b"KMAC256-XOF-v1");
    Update::update(&mut hasher, &(input.len() as u64).to_le_bytes());
    Update::update(&mut hasher, input);
    Update::update(&mut hasher, &(label.len() as u64).to_le_bytes());
    Update::update(&mut hasher, label);
    Update::update(&mut hasher, &(salt.len() as u64).to_le_bytes());
    Update::update(&mut hasher, salt);
    
    let mut reader = hasher.finalize_xof();
    let mut out = vec![0u8; output_len];
    XofReader::read(&mut reader, &mut out);
    out
}

/// KMAC256 tag generation (MAC)
/// Generates a 32-byte authentication tag
pub fn kmac256_tag(key: &[u8], label: &[u8], message: &[u8]) -> [u8; 32] {
    let mut hasher = Shake256::default();
    Update::update(&mut hasher, b"KMAC256-TAG-v1");
    Update::update(&mut hasher, &(key.len() as u64).to_le_bytes());
    Update::update(&mut hasher, key);
    Update::update(&mut hasher, &(label.len() as u64).to_le_bytes());
    Update::update(&mut hasher, label);
    Update::update(&mut hasher, &(message.len() as u64).to_le_bytes());
    Update::update(&mut hasher, message);
    
    let mut reader = hasher.finalize_xof();
    let mut out = [0u8; 32];
    XofReader::read(&mut reader, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kmac256_derive_key_deterministic() {
        let k1 = kmac256_derive_key(b"password", b"label", b"salt");
        let k2 = kmac256_derive_key(b"password", b"label", b"salt");
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_kmac256_xof_variable_length() {
        let out1 = kmac256_xof(b"input", b"label", b"salt", 16);
        let out2 = kmac256_xof(b"input", b"label", b"salt", 32);
        assert_eq!(out1.len(), 16);
        assert_eq!(out2.len(), 32);
        assert_eq!(&out2[..16], &out1[..]);
    }

    #[test]
    fn test_kmac256_tag_deterministic() {
        let t1 = kmac256_tag(b"key", b"label", b"message");
        let t2 = kmac256_tag(b"key", b"label", b"message");
        assert_eq!(t1, t2);
    }
}
