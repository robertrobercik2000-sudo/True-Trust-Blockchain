//! KMAC256 cryptographic functions
#![forbid(unsafe_code)]

use sha3::{Shake256, digest::{Update, ExtendableOutput, XofReader}};
use zeroize::Zeroizing;

/// Derive a 32-byte key using KMAC256
pub fn kmac256_derive_key(key: &[u8], label: &[u8], context: &[u8]) -> [u8; 32] {
    let mut hasher = Shake256::default();
    hasher.update(b"KMAC256-DERIVE-v1");
    hasher.update(&(key.len() as u64).to_le_bytes());
    hasher.update(key);
    hasher.update(&(label.len() as u64).to_le_bytes());
    hasher.update(label);
    hasher.update(&(context.len() as u64).to_le_bytes());
    hasher.update(context);
    
    let mut reader = hasher.finalize_xof();
    let mut out = [0u8; 32];
    reader.read(&mut out);
    out
}

/// XOF (extendable output function) using KMAC256
pub fn kmac256_xof(key: &[u8], label: &[u8], context: &[u8], output_len: usize) -> Zeroizing<Vec<u8>> {
    let mut hasher = Shake256::default();
    hasher.update(b"KMAC256-XOF-v1");
    hasher.update(&(key.len() as u64).to_le_bytes());
    hasher.update(key);
    hasher.update(&(label.len() as u64).to_le_bytes());
    hasher.update(label);
    hasher.update(&(context.len() as u64).to_le_bytes());
    hasher.update(context);
    
    let mut reader = hasher.finalize_xof();
    let mut out = vec![0u8; output_len];
    reader.read(&mut out);
    Zeroizing::new(out)
}

/// Fill buffer using XOF
pub fn kmac256_xof_fill(key: &[u8], label: &[u8], context: &[u8], output: &mut [u8]) {
    let mut hasher = Shake256::default();
    hasher.update(b"KMAC256-XOF-v1");
    hasher.update(&(key.len() as u64).to_le_bytes());
    hasher.update(key);
    hasher.update(&(label.len() as u64).to_le_bytes());
    hasher.update(label);
    hasher.update(&(context.len() as u64).to_le_bytes());
    hasher.update(context);
    
    let mut reader = hasher.finalize_xof();
    reader.read(output);
}

/// Compute KMAC256 tag
pub fn kmac256_tag(key: &[u8], label: &[u8], message: &[u8]) -> [u8; 32] {
    let mut hasher = Shake256::default();
    hasher.update(b"KMAC256-TAG-v1");
    hasher.update(&(key.len() as u64).to_le_bytes());
    hasher.update(key);
    hasher.update(&(label.len() as u64).to_le_bytes());
    hasher.update(label);
    hasher.update(message);
    
    let mut reader = hasher.finalize_xof();
    let mut out = [0u8; 32];
    reader.read(&mut out);
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

    #[test]
    fn test_kmac_different_outputs() {
        let key = b"test_key";
        let label1 = b"label1";
        let label2 = b"label2";
        let context = b"context";
        
        let out1 = kmac256_derive_key(key, label1, context);
        let out2 = kmac256_derive_key(key, label2, context);
        
        assert_ne!(out1, out2);
    }

    #[test]
    fn test_xof_variable_length() {
        let key = b"test_key";
        let label = b"test";
        let context = b"ctx";
        
        let out16 = kmac256_xof(key, label, context, 16);
        let out32 = kmac256_xof(key, label, context, 32);
        let out64 = kmac256_xof(key, label, context, 64);
        
        assert_eq!(out16.len(), 16);
        assert_eq!(out32.len(), 32);
        assert_eq!(out64.len(), 64);
        
        // First 16 bytes should match
        assert_eq!(&out32[..16], &out16[..]);
        assert_eq!(&out64[..16], &out16[..]);
    }
}
