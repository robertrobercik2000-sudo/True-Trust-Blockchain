//! Transcript Construction for Quantum Hints
//! 
//! This module provides transcript binding for all hint parameters
//! to prevent mix-and-match attacks.

use crate::keysearch::HintPayloadV1;
use super::FalconError;
use crate::crypto::kmac::{kmac256_derive_key, kmac256_xof_fill};
use chacha20poly1305::{XChaCha20Poly1305, aead::{Aead, KeyInit}, XNonce};

/// Construct transcript binding all hint parameters
/// 
/// Binds: c_out, epoch, timestamp, kem_ct, x25519_eph_pub, sender_falcon_pk
pub fn transcript(
    epoch: u64,
    timestamp: u64,
    c_out: &[u8; 32],
    kem_ct: &[u8],
    x25519_eph_pub: &[u8; 32],
    sender_falcon_pk: &[u8],
) -> Vec<u8> {
    let mut t = Vec::with_capacity(
        8 + 32 + 8 + 8 + 4 + kem_ct.len() + 32 + sender_falcon_pk.len()
    );
    
    t.extend_from_slice(b"QHINT.v1");              // domain
    t.extend_from_slice(c_out);                     // commitment
    t.extend_from_slice(&epoch.to_le_bytes());      // epoch
    t.extend_from_slice(&timestamp.to_le_bytes());  // timestamp
    t.extend_from_slice(&(kem_ct.len() as u32).to_le_bytes());
    t.extend_from_slice(kem_ct);                    // KEM ciphertext
    t.extend_from_slice(x25519_eph_pub);            // X25519 ephemeral
    t.extend_from_slice(sender_falcon_pk);          // Falcon PK
    
    t
}

/// AEAD encrypt payload with XChaCha20-Poly1305
/// 
/// Key and nonce derived from shared secret via KMAC
pub fn aead_encrypt(
    ss_h: &[u8; 32],
    aad: &[u8],
    payload: &HintPayloadV1,
) -> Result<Vec<u8>, FalconError> {
    // Derive key and nonce from shared secret
    let key = kmac256_derive_key(ss_h, b"QH/AEAD/Key", b"");
    let mut nonce24 = [0u8; 24];
    kmac256_xof_fill(ss_h, b"QH/AEAD/Nonce24", b"", &mut nonce24);
    
    // Create cipher
    let cipher = XChaCha20Poly1305::new_from_slice(&key)
        .map_err(|_| FalconError::SerializationFailed)?;
    
    // Serialize payload
    let plaintext = bincode::serialize(payload)
        .map_err(|_| FalconError::SerializationFailed)?;
    
    // Encrypt with AAD
    let ciphertext = cipher
        .encrypt(
            &XNonce::from(nonce24),
            chacha20poly1305::aead::Payload {
                msg: &plaintext,
                aad,
            },
        )
        .map_err(|_| FalconError::SerializationFailed)?;
    
    Ok(ciphertext)
}

/// AEAD decrypt payload with XChaCha20-Poly1305
/// 
/// Returns None if decryption or deserialization fails
pub fn aead_decrypt(
    ss_h: &[u8; 32],
    aad: &[u8],
    ciphertext: &[u8],
) -> Option<HintPayloadV1> {
    // Derive key and nonce (same as encrypt)
    let key = kmac256_derive_key(ss_h, b"QH/AEAD/Key", b"");
    let mut nonce24 = [0u8; 24];
    kmac256_xof_fill(ss_h, b"QH/AEAD/Nonce24", b"", &mut nonce24);
    
    // Create cipher
    let cipher = XChaCha20Poly1305::new_from_slice(&key).ok()?;
    
    // Decrypt with AAD
    let plaintext = cipher
        .decrypt(
            &XNonce::from(nonce24),
            chacha20poly1305::aead::Payload {
                msg: ciphertext,
                aad,
            },
        )
        .ok()?;
    
    // Deserialize payload
    bincode::deserialize(&plaintext).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transcript_deterministic() {
        let epoch = 42;
        let timestamp = 1234567890;
        let c_out = [0x42u8; 32];
        let kem_ct = vec![0x99u8; 1088]; // Kyber768 CT size
        let x25519_eph = [0xAAu8; 32];
        let falcon_pk = vec![0xBBu8; 897]; // Falcon512 PK size
        
        let t1 = transcript(epoch, timestamp, &c_out, &kem_ct, &x25519_eph, &falcon_pk);
        let t2 = transcript(epoch, timestamp, &c_out, &kem_ct, &x25519_eph, &falcon_pk);
        
        assert_eq!(t1, t2, "Transcript must be deterministic");
    }

    #[test]
    fn test_aead_roundtrip() {
        let ss_h = [0x42u8; 32];
        let aad = b"test AAD";
        
        let payload = HintPayloadV1 {
            r_blind: [0x99u8; 32],
            value: 1000,
            memo: vec![],
        };
        
        let ct = aead_encrypt(&ss_h, aad, &payload).unwrap();
        let recovered = aead_decrypt(&ss_h, aad, &ct).unwrap();
        
        assert_eq!(recovered.r_blind, payload.r_blind);
        assert_eq!(recovered.value, payload.value);
    }

    #[test]
    fn test_aead_wrong_aad_fails() {
        let ss_h = [0x42u8; 32];
        let aad = b"correct AAD";
        let wrong_aad = b"wrong AAD";
        
        let payload = HintPayloadV1 {
            r_blind: [0x99u8; 32],
            value: 1000,
            memo: vec![],
        };
        
        let ct = aead_encrypt(&ss_h, aad, &payload).unwrap();
        let result = aead_decrypt(&ss_h, wrong_aad, &ct);
        
        assert!(result.is_none(), "Wrong AAD should fail decryption");
    }
}
