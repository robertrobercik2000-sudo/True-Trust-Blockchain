//! Unified quantum-safe and traditional keysearch
#![forbid(unsafe_code)]

use super::falcon_integration::{QuantumKeySearchCtx, QuantumSafeHint, HintPayload};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret as X25519Secret};
use zeroize::Zeroizing;

/* ===== UNIFIED INTERFACE ===== */

pub struct UnifiedKeySearch {
    quantum_ctx: Option<QuantumKeySearchCtx>,
    x25519_secret: Zeroizing<[u8; 32]>,
    x25519_public: [u8; 32],
}

impl UnifiedKeySearch {
    pub fn new(master_seed: [u8; 32]) -> Self {
        let quantum_ctx = QuantumKeySearchCtx::new(master_seed).ok();
        
        let x25519_secret = Zeroizing::new(super::kmac::kmac256_derive_key(
            &master_seed,
            b"X25519_UNIFIED_v1",
            b"keysearch"
        ));
        let x25519_secret_scalar = X25519Secret::from(*x25519_secret);
        let x25519_public = X25519PublicKey::from(&x25519_secret_scalar).to_bytes();
        
        Self {
            quantum_ctx,
            x25519_secret,
            x25519_public,
        }
    }
    
    pub fn get_public_keys(&self) -> PublicKeys {
        PublicKeys {
            falcon_pk: self.quantum_ctx.as_ref().map(|ctx| ctx.get_falcon_public_key().to_vec()),
            x25519_pk: self.x25519_public,
        }
    }
    
    pub fn build_smart_hint(
        &mut self,
        recipient_keys: &PublicKeys,
        c_out: &[u8; 32],
        payload: &HintPayload,
    ) -> Result<SmartHint, BuildError> {
        if let (Some(quantum_ctx), Some(ref falcon_pk)) = 
            (&mut self.quantum_ctx, &recipient_keys.falcon_pk) 
        {
            match quantum_ctx.build_quantum_hint(
                falcon_pk,
                &recipient_keys.x25519_pk,
                c_out,
                payload,
            ) {
                Ok(quantum_hint) => return Ok(SmartHint::Quantum(quantum_hint)),
                Err(_) => {} // Fall through to traditional
            }
        }
        
        let traditional_hint = self.build_traditional_hint(&recipient_keys.x25519_pk, c_out, payload)?;
        Ok(SmartHint::Traditional(traditional_hint))
    }
    
    pub fn scan_smart<'a, I>(&mut self, outputs: I) -> Vec<FoundNote>
    where
        I: IntoIterator<Item = (&'a [u8; 32], &'a [u8])>,
    {
        let mut results = Vec::new();
        
        for (idx, (c_out, hint_data)) in outputs.into_iter().enumerate() {
            if let Ok(quantum_hint) = bincode::deserialize::<QuantumSafeHint>(hint_data) {
                if let Some(ref mut quantum_ctx) = self.quantum_ctx {
                    if let Some((payload, quantum_safe)) = quantum_ctx.verify_quantum_hint(&quantum_hint, c_out) {
                        results.push(FoundNote {
                            index: idx,
                            c_out: *c_out,
                            payload,
                            quantum_safe,
                            method: "falcon512".to_string(),
                        });
                        continue;
                    }
                }
            }
            
            if let Some(payload) = self.verify_traditional_hint(hint_data, c_out) {
                results.push(FoundNote {
                    index: idx,
                    c_out: *c_out,
                    payload,
                    quantum_safe: false,
                    method: "x25519".to_string(),
                });
            }
        }
        
        results
    }
    
    pub fn has_quantum_support(&self) -> bool {
        self.quantum_ctx.is_some()
    }
    
    /* ===== PRIVATE METHODS ===== */
    
    fn build_traditional_hint(
        &self,
        recipient_pk: &[u8; 32],
        c_out: &[u8; 32],
        payload: &HintPayload,
    ) -> Result<TraditionalHint, BuildError> {
        use aes_gcm_siv::{Aes256GcmSiv, KeyInit, Nonce, aead::Aead};
        use rand::RngCore;
        use rand::rngs::OsRng;
        
        let mut eph_secret_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut eph_secret_bytes);
        let eph_secret = X25519Secret::from(eph_secret_bytes);
        let eph_public = X25519PublicKey::from(&eph_secret);
        
        let recipient_x25519 = X25519PublicKey::from(*recipient_pk);
        let shared_secret = eph_secret.diffie_hellman(&recipient_x25519);
        
        let enc_key = super::kmac::kmac256_derive_key(
            shared_secret.as_bytes(),
            b"TRADITIONAL_HINT_v1",
            c_out
        );
        
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);
        
        let cipher = Aes256GcmSiv::new_from_slice(&enc_key)
            .map_err(|_| BuildError::EncryptionFailed)?;
        
        let plaintext = bincode::serialize(payload)
            .map_err(|_| BuildError::SerializationFailed)?;
        
        let ciphertext = cipher.encrypt(Nonce::from_slice(&nonce), aes_gcm_siv::aead::Payload {
            msg: &plaintext,
            aad: c_out,
        }).map_err(|_| BuildError::EncryptionFailed)?;
        
        Ok(TraditionalHint {
            eph_public: eph_public.to_bytes(),
            ciphertext,
            nonce,
        })
    }
    
    fn verify_traditional_hint(&self, hint_data: &[u8], c_out: &[u8; 32]) -> Option<HintPayload> {
        use aes_gcm_siv::{Aes256GcmSiv, KeyInit, Nonce, aead::Aead};
        
        let hint: TraditionalHint = bincode::deserialize(hint_data).ok()?;
        
        let my_secret = X25519Secret::from(*self.x25519_secret);
        let eph_public = X25519PublicKey::from(hint.eph_public);
        let shared_secret = my_secret.diffie_hellman(&eph_public);
        
        let dec_key = super::kmac::kmac256_derive_key(
            shared_secret.as_bytes(),
            b"TRADITIONAL_HINT_v1",
            c_out
        );
        
        let cipher = Aes256GcmSiv::new_from_slice(&dec_key).ok()?;
        let plaintext = cipher.decrypt(Nonce::from_slice(&hint.nonce), aes_gcm_siv::aead::Payload {
            msg: &hint.ciphertext,
            aad: c_out,
        }).ok()?;
        
        bincode::deserialize(&plaintext).ok()
    }
}

/* ===== DATA STRUCTURES ===== */

#[derive(Clone, Debug)]
pub struct PublicKeys {
    pub falcon_pk: Option<Vec<u8>>,
    pub x25519_pk: [u8; 32],
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum SmartHint {
    Quantum(QuantumSafeHint),
    Traditional(TraditionalHint),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TraditionalHint {
    pub eph_public: [u8; 32],
    pub ciphertext: Vec<u8>,
    pub nonce: [u8; 12],
}

#[derive(Clone, Debug)]
pub struct FoundNote {
    pub index: usize,
    pub c_out: [u8; 32],
    pub payload: HintPayload,
    pub quantum_safe: bool,
    pub method: String,
}

/* ===== ERROR HANDLING ===== */

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("Encryption failed")]
    EncryptionFailed,
    #[error("Serialization failed")]
    SerializationFailed,
}

/* ===== TESTS ===== */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unified_creation() {
        let seed = [0x42u8; 32];
        let unified = UnifiedKeySearch::new(seed);
        
        let keys = unified.get_public_keys();
        assert_eq!(keys.x25519_pk.len(), 32);
        println!("Quantum support: {}", unified.has_quantum_support());
    }
    
    #[test]
    fn test_traditional_hint_roundtrip() {
        let alice_seed = [0x01u8; 32];
        let bob_seed = [0x02u8; 32];
        
        let mut alice = UnifiedKeySearch::new(alice_seed);
        let mut bob = UnifiedKeySearch::new(bob_seed);
        
        let bob_keys = bob.get_public_keys();
        let c_out = [0x42u8; 32];
        
        let payload = HintPayload {
            r_blind: [0x99u8; 32],
            value: Some(1_000_000),
            memo: b"test traditional".to_vec(),
        };
        
        let hint = alice.build_smart_hint(&bob_keys, &c_out, &payload).unwrap();
        let hint_bytes = bincode::serialize(&hint).unwrap();
        
        let results = bob.scan_smart(vec![(&c_out, hint_bytes.as_slice())]);
        
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].payload.value, Some(1_000_000));
    }
}
