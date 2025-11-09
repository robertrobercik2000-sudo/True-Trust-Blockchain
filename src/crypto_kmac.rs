//! KMAC256 and SHAKE256 functions for keysearch
#![forbid(unsafe_code)]

use sha3::{Shake256, digest::{Update, ExtendableOutput, XofReader}};

/// KMAC256 XOF - fill output buffer
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

/// SHAKE256 hash (32 bytes output) for multiple inputs
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
    fn test_kmac_xof_fill() {
        let key = b"test_key";
        let label = b"test_label";
        let context = b"test_context";
        let mut out1 = [0u8; 32];
        let mut out2 = [0u8; 32];
        
        kmac256_xof_fill(key, label, context, &mut out1);
        kmac256_xof_fill(key, label, context, &mut out2);
        
        assert_eq!(out1, out2);
    }

    #[test]
    fn test_shake256_32() {
        let h1 = shake256_32(&[b"test"]);
        let h2 = shake256_32(&[b"test"]);
        assert_eq!(h1, h2);
        
        let h3 = shake256_32(&[b"different"]);
        assert_ne!(h1, h3);
    }
}
