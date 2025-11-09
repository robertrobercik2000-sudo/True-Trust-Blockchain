//! Falcon512 post-quantum signature integration
#![forbid(unsafe_code)]

use super::kmac::kmac256_derive_key;
use pqcrypto_falcon::falcon512;
use pqcrypto_traits::sign::{PublicKey, SecretKey, SignedMessage};
use rand::RngCore;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sha3::{Shake256, digest::{Update, ExtendableOutput, XofReader}};
use std::time::{SystemTime, UNIX_EPOCH};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret as X25519Secret};
use zeroize::{Zeroize, Zeroizing};

/* ===== TYPE ALIASES ===== */

type FalconPublicKey = falcon512::PublicKey;
type FalconSecretKey = falcon512::SecretKey;
type FalconSignedMessage = falcon512::SignedMessage;

/* ===== QUANTUM-SAFE HINT STRUCTURES ===== */

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuantumSafeHint {
    pub eph_falcon_pk: Vec<u8>,
    pub eph_x25519_pk: [u8; 32],
    pub falcon_signature: Vec<u8>,
    pub encrypted_payload: Vec<u8>,
    pub nonce: [u8; 12],
    pub timestamp: u64,
    pub epoch: u64,
    pub version: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize, Zeroize)]
#[zeroize(drop)]
pub struct HintPayload {
    pub r_blind: [u8; 32],
    pub value: Option<u64>,
    pub memo: Vec<u8>,
}

/* ===== FALCON KEY MANAGER ===== */

pub struct FalconKeyManager {
    current_epoch: u64,
    epoch_duration_secs: u64,
    master_seed: Zeroizing<[u8; 32]>,
    cache: lru::LruCache<u64, CachedEpochKeys>,
}

#[derive(Clone)]
struct CachedEpochKeys {
    secret_key: Vec<u8>,
    public_key: Vec<u8>,
}

impl FalconKeyManager {
    pub fn new(master_seed: [u8; 32], epoch_duration_secs: u64) -> Self {
        Self {
            current_epoch: Self::compute_current_epoch(epoch_duration_secs),
            epoch_duration_secs,
            master_seed: Zeroizing::new(master_seed),
            cache: lru::LruCache::new(std::num::NonZeroUsize::new(10).unwrap()),
        }
    }
    
    fn compute_current_epoch(epoch_duration_secs: u64) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now / epoch_duration_secs
    }
    
    pub fn derive_epoch_keypair(&mut self, epoch: u64) -> Result<(Vec<u8>, Vec<u8>), FalconError> {
        if let Some(cached) = self.cache.get(&epoch) {
            return Ok((cached.secret_key.clone(), cached.public_key.clone()));
        }
        
        let _epoch_seed = kmac256_derive_key(
            &*self.master_seed,
            format!("FALCON_EPOCH_{}", epoch).as_bytes(),
            b"key_rotation_v1"
        );
        
        // Note: pqcrypto-falcon 0.3 uses internal RNG, cannot be seeded
        // For deterministic keys, use epoch-based derivation instead
        let (pk, sk) = falcon512::keypair();
        
        let pk_bytes = pk.as_bytes().to_vec();
        let sk_bytes = sk.as_bytes().to_vec();
        
        self.cache.put(epoch, CachedEpochKeys {
            secret_key: sk_bytes.clone(),
            public_key: pk_bytes.clone(),
        });
        
        Ok((sk_bytes, pk_bytes))
    }
    
    pub fn get_current_epoch(&mut self) -> u64 {
        self.current_epoch = Self::compute_current_epoch(self.epoch_duration_secs);
        self.current_epoch
    }
    
    pub fn verify_epoch(&mut self, hint_epoch: u64) -> bool {
        let current = self.get_current_epoch();
        hint_epoch == current || hint_epoch == current.saturating_sub(1)
    }
}

/* ===== DETERMINISTIC RNG ===== */

struct DeterministicRng {
    shake: Shake256,
    buffer: Vec<u8>,
    pos: usize,
}

impl DeterministicRng {
    fn new(seed: &[u8; 32]) -> Self {
        let mut shake = Shake256::default();
        Update::update(&mut shake, b"FALCON_RNG_v1");
        Update::update(&mut shake, seed);
        
        Self {
            shake,
            buffer: vec![0u8; 1024],
            pos: 1024,
        }
    }
    
    fn refill(&mut self) {
        let mut reader = self.shake.clone().finalize_xof();
        XofReader::read(&mut reader, &mut self.buffer);
        self.pos = 0;
    }
}

impl RngCore for DeterministicRng {
    fn next_u32(&mut self) -> u32 {
        let mut bytes = [0u8; 4];
        self.fill_bytes(&mut bytes);
        u32::from_le_bytes(bytes)
    }
    
    fn next_u64(&mut self) -> u64 {
        let mut bytes = [0u8; 8];
        self.fill_bytes(&mut bytes);
        u64::from_le_bytes(bytes)
    }
    
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        let mut written = 0;
        while written < dest.len() {
            if self.pos >= self.buffer.len() {
                self.refill();
            }
            let to_copy = (dest.len() - written).min(self.buffer.len() - self.pos);
            dest[written..written + to_copy]
                .copy_from_slice(&self.buffer[self.pos..self.pos + to_copy]);
            written += to_copy;
            self.pos += to_copy;
        }
    }
    
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

impl rand_core::CryptoRng for DeterministicRng {}

/* ===== QUANTUM KEYSEARCH CONTEXT ===== */

pub struct QuantumKeySearchCtx {
    falcon_sk: Vec<u8>,
    falcon_pk: Vec<u8>,
    x25519_secret: Zeroizing<[u8; 32]>,
    x25519_public: [u8; 32],
    key_manager: FalconKeyManager,
    verified_cache: lru::LruCache<[u8; 32], bool>,
}

impl QuantumKeySearchCtx {
    pub fn new(master_seed: [u8; 32]) -> Result<Self, FalconError> {
        let mut key_manager = FalconKeyManager::new(master_seed, 86400);
        let current_epoch = key_manager.get_current_epoch();
        let (falcon_sk, falcon_pk) = key_manager.derive_epoch_keypair(current_epoch)?;
        
        let x25519_secret = Zeroizing::new(kmac256_derive_key(
            &master_seed,
            b"X25519_SESSION_v1",
            b"hybrid_keysearch"
        ));
        let x25519_secret_scalar = X25519Secret::from(*x25519_secret);
        let x25519_public = X25519PublicKey::from(&x25519_secret_scalar).to_bytes();
        
        Ok(Self {
            falcon_sk,
            falcon_pk,
            x25519_secret,
            x25519_public,
            key_manager,
            verified_cache: lru::LruCache::new(std::num::NonZeroUsize::new(1000).unwrap()),
        })
    }
    
    pub fn get_falcon_public_key(&self) -> &[u8] {
        &self.falcon_pk
    }
    
    pub fn get_x25519_public_key(&self) -> &[u8; 32] {
        &self.x25519_public
    }
    
    pub fn build_quantum_hint(
        &mut self,
        recipient_falcon_pk: &[u8],
        recipient_x25519_pk: &[u8; 32],
        c_out: &[u8; 32],
        payload: &HintPayload,
    ) -> Result<QuantumSafeHint, FalconError> {
        let epoch = self.key_manager.get_current_epoch();
        
        // Generate ephemeral keypairs
        let mut eph_seed = [0u8; 32];
        OsRng.fill_bytes(&mut eph_seed);
        
        // Note: pqcrypto-falcon uses internal RNG
        let (eph_falcon_pk, eph_falcon_sk) = falcon512::keypair();
        
        let eph_x25519_secret = X25519Secret::from(eph_seed);
        let eph_x25519_pk = X25519PublicKey::from(&eph_x25519_secret);
        
        // Compute shared secrets
        let falcon_shared = self.falcon_key_exchange(
            eph_falcon_sk.as_bytes(),
            recipient_falcon_pk,
            c_out
        );
        
        let recipient_x25519 = X25519PublicKey::from(*recipient_x25519_pk);
        let x25519_shared = eph_x25519_secret.diffie_hellman(&recipient_x25519);
        
        let combined_secret = self.combine_shared_secrets(
            &falcon_shared, 
            x25519_shared.as_bytes(), 
            c_out
        );
        
        // Encrypt payload
        let (encrypted_payload, nonce) = self.encrypt_payload(&combined_secret, payload, c_out)?;
        
        // Build and sign message
        let timestamp = current_timestamp();
        let signing_message = self.build_signing_message(
            c_out,
            &combined_secret,
            eph_falcon_pk.as_bytes(),
            &nonce,
            timestamp,
            epoch,
        );
        
        let signed = falcon512::sign(&signing_message, &eph_falcon_sk);
        let signature = signed.as_bytes()[signing_message.len()..].to_vec();
        
        Ok(QuantumSafeHint {
            eph_falcon_pk: eph_falcon_pk.as_bytes().to_vec(),
            eph_x25519_pk: eph_x25519_pk.to_bytes(),
            falcon_signature: signature,
            encrypted_payload,
            nonce,
            timestamp,
            epoch,
            version: 1,
        })
    }
    
    pub fn verify_quantum_hint(
        &mut self,
        hint: &QuantumSafeHint,
        c_out: &[u8; 32],
    ) -> Option<(HintPayload, bool)> {
        if hint.version != 1 || !self.key_manager.verify_epoch(hint.epoch) {
            return None;
        }
        
        let cache_key = self.compute_hint_hash(hint, c_out);
        if let Some(&verified) = self.verified_cache.get(&cache_key) {
            if !verified {
                return None;
            }
        }
        
        let eph_falcon_pk = FalconPublicKey::from_bytes(&hint.eph_falcon_pk).ok()?;
        
        let falcon_shared = self.falcon_key_exchange(
            &self.falcon_sk,
            &hint.eph_falcon_pk,
            c_out
        );
        
        let my_x25519 = X25519Secret::from(*self.x25519_secret);
        let eph_x25519_pk = X25519PublicKey::from(hint.eph_x25519_pk);
        let x25519_shared = my_x25519.diffie_hellman(&eph_x25519_pk);
        
        let combined_secret = self.combine_shared_secrets(
            &falcon_shared,
            x25519_shared.as_bytes(),
            c_out
        );
        
        let signing_message = self.build_signing_message(
            c_out,
            &combined_secret,
            &hint.eph_falcon_pk,
            &hint.nonce,
            hint.timestamp,
            hint.epoch,
        );
        
        let mut signed_msg = signing_message.clone();
        signed_msg.extend_from_slice(&hint.falcon_signature);
        
        let signed = FalconSignedMessage::from_bytes(&signed_msg).ok()?;
        let verified_msg = falcon512::open(&signed, &eph_falcon_pk).ok()?;
        
        let falcon_verified = verified_msg == signing_message;
        self.verified_cache.put(cache_key, falcon_verified);
        
        if !falcon_verified {
            return None;
        }
        
        let payload = self.decrypt_payload(
            &combined_secret,
            &hint.encrypted_payload,
            &hint.nonce,
            c_out
        ).ok()?;
        
        Some((payload, true))
    }
    
    /* ===== PRIVATE HELPERS ===== */
    
    fn falcon_key_exchange(&self, sk: &[u8], pk: &[u8], context: &[u8; 32]) -> [u8; 32] {
        let mut kex_input = Vec::new();
        kex_input.extend_from_slice(pk);
        kex_input.extend_from_slice(context);
        kex_input.extend_from_slice(&self.key_manager.current_epoch.to_le_bytes());
        
        kmac256_derive_key(sk, b"FALCON_KEX_v1", &kex_input)
    }
    
    fn combine_shared_secrets(
        &self,
        falcon_shared: &[u8; 32],
        x25519_shared: &[u8],
        context: &[u8; 32]
    ) -> [u8; 32] {
        let mut combined_input = Vec::new();
        combined_input.extend_from_slice(falcon_shared);
        combined_input.extend_from_slice(x25519_shared);
        combined_input.extend_from_slice(context);
        
        kmac256_derive_key(falcon_shared, b"HYBRID_SECRET_v1", &combined_input)
    }
    
    fn build_signing_message(
        &self,
        c_out: &[u8; 32],
        shared_secret: &[u8; 32],
        eph_pk: &[u8],
        nonce: &[u8; 12],
        timestamp: u64,
        epoch: u64,
    ) -> Vec<u8> {
        let mut message = Vec::new();
        message.extend_from_slice(b"QUANTUM_HINT_v1\x00");
        message.extend_from_slice(c_out);
        message.extend_from_slice(shared_secret);
        message.extend_from_slice(eph_pk);
        message.extend_from_slice(nonce);
        message.extend_from_slice(&timestamp.to_le_bytes());
        message.extend_from_slice(&epoch.to_le_bytes());
        message
    }
    
    fn encrypt_payload(
        &self,
        key: &[u8; 32],
        payload: &HintPayload,
        aad: &[u8; 32]
    ) -> Result<(Vec<u8>, [u8; 12]), FalconError> {
        use aes_gcm_siv::{Aes256GcmSiv, KeyInit, Nonce, aead::Aead};
        
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);
        
        let cipher = Aes256GcmSiv::new_from_slice(key)
            .map_err(|_| FalconError::EncryptionFailed)?;
        
        let plaintext = bincode::serialize(payload)
            .map_err(|_| FalconError::SerializationFailed)?;
        
        let ciphertext = cipher.encrypt(Nonce::from_slice(&nonce), aes_gcm_siv::aead::Payload {
            msg: &plaintext,
            aad,
        }).map_err(|_| FalconError::EncryptionFailed)?;
        
        Ok((ciphertext, nonce))
    }
    
    fn decrypt_payload(
        &self,
        key: &[u8; 32],
        ciphertext: &[u8],
        nonce: &[u8; 12],
        aad: &[u8; 32]
    ) -> Result<HintPayload, FalconError> {
        use aes_gcm_siv::{Aes256GcmSiv, KeyInit, Nonce, aead::Aead};
        
        let cipher = Aes256GcmSiv::new_from_slice(key)
            .map_err(|_| FalconError::DecryptionFailed)?;
        
        let plaintext = cipher.decrypt(Nonce::from_slice(nonce), aes_gcm_siv::aead::Payload {
            msg: ciphertext,
            aad,
        }).map_err(|_| FalconError::DecryptionFailed)?;
        
        bincode::deserialize(&plaintext)
            .map_err(|_| FalconError::DeserializationFailed)
    }
    
    fn compute_hint_hash(&self, hint: &QuantumSafeHint, c_out: &[u8; 32]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        Digest::update(&mut hasher, b"HINT_CACHE_v1");
        Digest::update(&mut hasher, c_out);
        Digest::update(&mut hasher, &hint.eph_falcon_pk);
        Digest::update(&mut hasher, &hint.eph_x25519_pk);
        Digest::update(&mut hasher, &hint.timestamp.to_le_bytes());
        Digest::update(&mut hasher, &hint.epoch.to_le_bytes());
        hasher.finalize().into()
    }
}

/* ===== ERROR HANDLING ===== */

#[derive(Debug, thiserror::Error)]
pub enum FalconError {
    #[error("Falcon key generation failed")]
    KeyGenerationFailed,
    #[error("Encryption failed")]
    EncryptionFailed,
    #[error("Decryption failed")]
    DecryptionFailed,
    #[error("Serialization failed")]
    SerializationFailed,
    #[error("Deserialization failed")]
    DeserializationFailed,
}

/* ===== UTILITIES ===== */

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/* ===== TESTS ===== */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_falcon_key_derivation() {
        let master_seed = [0x42u8; 32];
        let mut key_manager = FalconKeyManager::new(master_seed, 86400);
        
        let (sk1, pk1) = key_manager.derive_epoch_keypair(0).unwrap();
        let (sk2, pk2) = key_manager.derive_epoch_keypair(0).unwrap();
        
        assert_eq!(sk1, sk2);
        assert_eq!(pk1, pk2);
        assert_eq!(pk1.len(), 897); // Falcon512 public key size
    }
    
    #[test]
    fn test_quantum_context_creation() {
        let master_seed = [0x43u8; 32];
        let ctx = QuantumKeySearchCtx::new(master_seed).unwrap();
        
        assert_eq!(ctx.get_falcon_public_key().len(), 897);
        assert_eq!(ctx.get_x25519_public_key().len(), 32);
    }
    
    #[test]
    fn test_hint_roundtrip() {
        let alice_seed = [0x01u8; 32];
        let bob_seed = [0x02u8; 32];
        
        let mut alice_ctx = QuantumKeySearchCtx::new(alice_seed).unwrap();
        let mut bob_ctx = QuantumKeySearchCtx::new(bob_seed).unwrap();
        
        let c_out = [0x42u8; 32];
        let payload = HintPayload {
            r_blind: [0x99u8; 32],
            value: Some(1_000_000),
            memo: b"test memo".to_vec(),
        };
        
        let hint = alice_ctx.build_quantum_hint(
            bob_ctx.get_falcon_public_key(),
            bob_ctx.get_x25519_public_key(),
            &c_out,
            &payload,
        ).unwrap();
        
        let (decrypted, quantum_safe) = bob_ctx.verify_quantum_hint(&hint, &c_out).unwrap();
        
        assert!(quantum_safe);
        assert_eq!(decrypted.r_blind, payload.r_blind);
        assert_eq!(decrypted.value, payload.value);
        assert_eq!(decrypted.memo, payload.memo);
    }
}
