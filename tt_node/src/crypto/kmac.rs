//! KMAC256 cryptographic primitives
#![forbid(unsafe_code)]

use sha3::{Shake256, digest::{Update, ExtendableOutput, XofReader}};
use zeroize::Zeroizing;

/// Derive a 32-byte key using KMAC256
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

/// Fill buffer using XOF
pub fn kmac256_xof_fill(key: &[u8], label: &[u8], context: &[u8], output: &mut [u8]) {
    let mut hasher = Shake256::default();
    Update::update(&mut hasher, b"KMAC256-XOF-v1");
    Update::update(&mut hasher, &(key.len() as u64).to_le_bytes());
    Update::update(&mut hasher, key);
    Update::update(&mut hasher, &(label.len() as u64).to_le_bytes());
    Update::update(&mut hasher, label);
    Update::update(&mut hasher, &(context.len() as u64).to_le_bytes());
    Update::update(&mut hasher, context);
    
    let mut reader = hasher.finalize_xof();
    XofReader::read(&mut reader, output);
}

/// XOF returning Zeroizing vector
pub fn kmac256_xof(key: &[u8], label: &[u8], context: &[u8], output_len: usize) -> Zeroizing<Vec<u8>> {
    let mut out = vec![0u8; output_len];
    kmac256_xof_fill(key, label, context, &mut out);
    Zeroizing::new(out)
}

/// Generate MAC tag using KMAC256
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

/// SHAKE256 hash (32 bytes output) for multiple inputs
/// 
/// Used by keysearch for commitment verification
pub fn shake256_32(inputs: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Shake256::default();
    Update::update(&mut hasher, b"SHAKE256_32");
    for input in inputs {
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
    fn test_kmac_deterministic() {
        let key = b"test_key";
        let label = b"test_label";
        let context = b"test_context";
        
        let out1 = kmac256_derive_key(key, label, context);
        let out2 = kmac256_derive_key(key, label, context);
        
        assert_eq!(out1, out2);
    }
}