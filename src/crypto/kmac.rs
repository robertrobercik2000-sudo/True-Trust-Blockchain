use sha3::{
    digest::{ExtendableOutput, Update, XofReader},
    Shake256,
};

const DOMAIN_KDF: &[u8] = b"KMAC256-DERIVE-v1";
const DOMAIN_TAG: &[u8] = b"KMAC256-TAG-v1";
const DOMAIN_XOF: &[u8] = b"KMAC256-XOF-v1";

fn kmac256_core(parts: &[&[u8]], dst: &[u8], out_len: usize) -> Vec<u8> {
    let mut hasher = Shake256::default();
    for part in parts {
        hasher.update(&(part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    hasher.update(&(dst.len() as u64).to_le_bytes());
    hasher.update(dst);

    let mut reader = hasher.finalize_xof();
    let mut out = vec![0u8; out_len];
    reader.read(&mut out);
    out
}

pub fn kmac256_derive_key(input: &[u8], label: &[u8], salt: &[u8]) -> [u8; 32] {
    let mut parts = Vec::with_capacity(4);
    parts.push(DOMAIN_KDF);
    parts.push(label);
    parts.push(salt);
    parts.push(input);
    let out = kmac256_core(&parts, b"", 32);
    out.try_into().expect("xof length")
}

pub fn kmac256_tag(key: &[u8], label: &[u8], msg: &[u8]) -> [u8; 32] {
    let parts = [DOMAIN_TAG, label, key, msg];
    let out = kmac256_core(&parts, b"", 32);
    out.try_into().expect("xof length")
}

pub fn kmac256_xof(input: &[u8], label: &[u8], salt: &[u8], out_len: usize) -> Vec<u8> {
    let parts = [DOMAIN_XOF, label, salt, input];
    kmac256_core(&parts, b"", out_len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kdf_stability() {
        let key1 = kmac256_derive_key(b"input", b"label", b"salt");
        let key2 = kmac256_derive_key(b"input", b"label", b"salt");
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_xof_length() {
        let mask = kmac256_xof(b"input", b"label", b"salt", 48);
        assert_eq!(mask.len(), 48);
    }
}
