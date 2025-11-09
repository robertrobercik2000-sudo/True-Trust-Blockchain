//! KMAC256 helpers built on top of SHAKE256.
//!
//! Provides simple primitives for deriving symmetric keys, generating
//! authentication tags, and producing variable-length masks/XOF outputs.

use sha3::{
    digest::{ExtendableOutput, Update, XofReader},
    Shake256,
};

const KMAC_DOMAIN: &[u8] = b"TT-KMAC256.v1";

fn write_len_prefixed(hasher: &mut Shake256, data: &[u8]) {
    let len = data.len() as u64;
    hasher.update(&len.to_le_bytes());
    hasher.update(data);
}

fn kmac256_fill(key: &[u8], label: &[u8], data: &[u8], out: &mut [u8]) {
    let mut hasher = Shake256::default();
    hasher.update(KMAC_DOMAIN);
    write_len_prefixed(&mut hasher, key);
    write_len_prefixed(&mut hasher, label);
    write_len_prefixed(&mut hasher, data);

    let mut reader = hasher.finalize_xof();
    reader.read(out);
}

/// Derive a 32-byte symmetric key using KMAC256.
pub fn kmac256_derive_key(key_material: &[u8], label: &[u8], salt: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    kmac256_fill(key_material, label, salt, &mut out);
    out
}

/// Produce a 32-byte authentication tag using KMAC256.
pub fn kmac256_tag(key: &[u8], label: &[u8], message: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    kmac256_fill(key, label, message, &mut out);
    out
}

/// Generate a variable-length mask/output using KMAC256 XOF mode.
pub fn kmac256_xof(key: &[u8], label: &[u8], nonce: &[u8], out_len: usize) -> Vec<u8> {
    if out_len == 0 {
        return Vec::new();
    }
    let mut out = vec![0u8; out_len];
    kmac256_fill(key, label, nonce, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_is_deterministic() {
        let k1 = kmac256_derive_key(b"master key", b"TT-TEST", b"salt");
        let k2 = kmac256_derive_key(b"master key", b"TT-TEST", b"salt");
        assert_eq!(k1, k2);
    }

    #[test]
    fn derive_changes_with_label() {
        let k1 = kmac256_derive_key(b"master key", b"TT-TEST-A", b"salt");
        let k2 = kmac256_derive_key(b"master key", b"TT-TEST-B", b"salt");
        assert_ne!(k1, k2);
    }

    #[test]
    fn tag_changes_with_message() {
        let t1 = kmac256_tag(b"key", b"LABEL", b"msg1");
        let t2 = kmac256_tag(b"key", b"LABEL", b"msg2");
        assert_ne!(t1, t2);
    }

    #[test]
    fn xof_len_matches_request() {
        let out = kmac256_xof(b"key", b"LABEL", b"nonce", 48);
        assert_eq!(out.len(), 48);
    }
}
