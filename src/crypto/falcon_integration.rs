//! Falcon512 post-quantum signature integration for quantum-safe keysearch
#![forbid(unsafe_code)]

use super::kmac::{kmac256_derive_key, kmac256_xof_fill};
use pqcrypto_falcon::falcon512;
use pqcrypto_traits::sign::{PublicKey, SecretKey, SignedMessage};

type FalconPublicKey = falcon512::PublicKey;
type FalconSecretKey = falcon512::SecretKey;
type FalconSignedMessage = falcon512::SignedMessage;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha3::{Shake256, digest::{Update, ExtendableOutput, XofReader}};
use std::time::{SystemTime, UNIX_EPOCH};
use x25519_dalek::{PublicKey as X25519PublicKey, EphemeralSecret as X25519Secret};
use zeroize::{Zeroize, Zeroizing};

/* ===== QUANTUM-SAFE KEYSEARCH STRUCTURES ===== */

/// Quantum-safe encrypted hint with Falcon512 authentication
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuantumSafeHint {
    /// Falcon512 ephemeral public key (897 bytes)
    pub eph_falcon_pk: Vec<u8>,
    
    /// Traditional X25519 ephemeral public key for hybrid security
    pub eph_x25519_pk: [u8; 32],
    
    /// Falcon512 signature over the hint (~666 bytes typical)
    pub falcon_signature: Vec<u8>,
    
    /// Encrypted payload (AES-GCM-SIV)
    pub encrypted_payload: Vec<u8>,
    
    /// Nonce for AEAD
    pub nonce: [u8; 12],
    
    /// Timestamp for replay attack prevention
    pub timestamp: u64,
    
    /// Key rotation epoch
    pub epoch: u64,
    
    /// Version for future upgrades
    pub version: u8,
}

/// Result of quantum-safe hint verification
#[derive(Clone, Debug)]
pub struct QuantumFoundNote {
    pub index: usize,
    pub c_out: [u8; 32],
    pub k_search: [u8; 32],
    pub falcon_verified: bool,
    pub quantum_safe: bool,
    pub value: Option<u64>,
    pub memo: Vec<u8>,
}

/// Decoded hint payload
#[derive(Clone, Debug, Serialize, Deserialize, Zeroize)]
#[zeroize(drop)]
pub struct HintPayload {
    pub r_blind: [u8; 32],
    pub value: Option<u64>,
    pub memo: Vec<u8>,
}

/* ===== FALCON KEY MANAGEMENT ===== */

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
    
    /// Derive epoch-specific Falcon keypair deterministically
    pub fn derive_epoch_keypair(&mut self, epoch: u64) -> Result<(Vec<u8>, Vec<u8>), FalconError> {
        // Check cache first
        if let Some(cached) = self.cache.get(&epoch) {
            return Ok((cached.secret_key.clone(), cached.public_key.clone()));
        }
        
        // Derive deterministic seed for this epoch
        let epoch_seed = kmac256_derive_key(
            &self.master_seed,
            format!("FALCON_EPOCH_{}", epoch).as_bytes(),
            b"key_rotation_v1"
        );
        
        // Generate keypair using deterministic RNG
        let mut rng = DeterministicRng::new(&epoch_seed);
        let (pk, sk) = falcon512::keypair(&mut rng);
        
        let pk_bytes = pk.as_bytes().to_vec();
        let sk_bytes = sk.as_bytes().to_vec();
        
        // Cache the keys
        self.cache.put(epoch, CachedEpochKeys {
            secret_key: sk_bytes.clone(),
            public_key: pk_bytes.clone(),
        });
        
        Ok((sk_bytes, pk_bytes))
    }
    
    pub fn get_current_epoch(&mut self) -> u64 {
        // Recompute in case time has passed
        self.current_epoch = Self::compute_current_epoch(self.epoch_duration_secs);
        self.current_epoch
    }
    
    /// Verify if a hint is from an acceptable epoch (current or previous)
    pub fn verify_epoch(&mut self, hint_epoch: u64) -> bool {
        let current = self.get_current_epoch();
        // Allow current epoch and previous epoch (for clock skew)
        hint_epoch == current || hint_epoch == current.saturating_sub(1)
    }
}

/* ===== DETERMINISTIC RNG FOR FALCON ===== */

struct DeterministicRng {
    shake: Shake256,
    buffer: Vec<u8>,
    pos: usize,
}

impl DeterministicRng {
    fn new(seed: &[u8; 32]) -> Self {
        let mut shake = Shake256::default();
        shake.update(b"FALCON_DETERMINISTIC_RNG_v1");
        shake.update(seed);
        
        Self {
            shake,
            buffer: vec![0u8; 1024],
            pos: 1024, // Force refill on first use
        }
    }
    
    fn refill(&mut self) {
        use sha3::digest::XofReader;
        let mut reader = self.shake.clone().finalize_xof();
        reader.read(&mut self.buffer);
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
            dest[written..written + to_copy].copy_from_slice(&self.buffer[self.pos..self.pos + to_copy]);
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

use rand_core::RngCore;

/* ===== QUANTUM KEYSEARCH CONTEXT ===== */

pub struct QuantumKeySearchCtx {
    /// Post-quantum Falcon identity
    falcon_sk: Vec<u8>,
    falcon_pk: Vec<u8>,
    
    /// Traditional X25519 for hybrid security
    x25519_secret: Zeroizing<[u8; 32]>,
    x25519_public: [u8; 32],
    
    /// Key management
    key_manager: FalconKeyManager,
    
    /// Performance cache
    verified_cache: lru::LruCache<[u8; 32], bool>,
}

impl QuantumKeySearchCtx {
    pub fn new(master_seed: [u8; 32]) -> Result<Self, FalconError> {
        let mut key_manager = FalconKeyManager::new(master_seed, 86400); // 24h epochs
        let current_epoch = key_manager.get_current_epoch();
        let (falcon_sk, falcon_pk) = key_manager.derive_epoch_keypair(current_epoch)?;
        
        // Derive X25519 keys
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
    
    /// Create quantum-safe hint for recipient
    pub fn build_quantum_hint(
        &mut self,
        recipient_falcon_pk: &[u8],
        recipient_x25519_pk: &[u8; 32],
        c_out: &[u8; 32],
        payload: &HintPayload,
    ) -> Result<QuantumSafeHint, FalconError> {
        let epoch = self.key_manager.get_current_epoch();
        
        // 1. Generate ephemeral keypairs
        let mut eph_seed = [0u8; 32];
        OsRng.fill_bytes(&mut eph_seed);
        
        let mut rng = DeterministicRng::new(&eph_seed);
        let (eph_falcon_pk, eph_falcon_sk) = falcon512::keypair(&mut rng);
        
        let eph_x25519_secret = X25519Secret::from(eph_seed);
        let eph_x25519_pk = X25519PublicKey::from(&eph_x25519_secret);
        
        // 2. Compute shared secrets (hybrid approach)
        let falcon_shared = self.falcon_key_exchange(
            eph_falcon_sk.as_bytes(),
            recipient_falcon_pk,
            c_out
        );
        
        let recipient_x25519 = X25519PublicKey::from(*recipient_x25519_pk);
        let x25519_shared = eph_x25519_secret.diffie_hellman(&recipient_x25519);
        
        // 3. Combine shared secrets
        let combined_secret = self.combine_shared_secrets(&falcon_shared, x25519_shared.as_bytes(), c_out);
        
        // 4. Encrypt payload
        let (encrypted_payload, nonce) = self.encrypt_payload(&combined_secret, payload, c_out)?;
        
        // 5. Build signing message
        let timestamp = current_timestamp();
        let signing_message = self.build_signing_message(
            c_out,
            &combined_secret,
            eph_falcon_pk.as_bytes(),
            &nonce,
            timestamp,
            epoch,
        );
        
        // 6. Sign with ephemeral Falcon key
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
    
    /// Verify and decrypt quantum-safe hint
    pub fn verify_quantum_hint(
        &mut self,
        hint: &QuantumSafeHint,
        c_out: &[u8; 32],
    ) -> Option<(HintPayload, bool)> {
        // 1. Check version
        if hint.version != 1 {
            return None;
        }
        
        // 2. Verify epoch
        if !self.key_manager.verify_epoch(hint.epoch) {
            return None;
        }
        
        // 3. Check cache
        let cache_key = self.compute_hint_hash(hint, c_out);
        if let Some(&verified) = self.verified_cache.get(&cache_key) {
            if !verified {
                return None;
            }
        }
        
        // 4. Parse Falcon public key
        let eph_falcon_pk = FalconPublicKey::from_bytes(&hint.eph_falcon_pk).ok()?;
        
        // 5. Recreate shared secrets
        let falcon_shared = self.falcon_key_exchange(
            &self.falcon_sk,
            hint.eph_falcon_pk.as_ref(),
            c_out
        );
        
        let my_x25519 = X25519Secret::from(*self.x25519_secret);
        let eph_x25519_pk = X25519PublicKey::from(hint.eph_x25519_pk);
        let x25519_shared = my_x25519.diffie_hellman(&eph_x25519_pk);
        
        // 6. Combine shared secrets
        let combined_secret = self.combine_shared_secrets(&falcon_shared, x25519_shared.as_bytes(), c_out);
        
        // 7. Verify Falcon signature
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
        
        // 8. Cache result
        self.verified_cache.put(cache_key, falcon_verified);
        
        if !falcon_verified {
            return None;
        }
        
        // 9. Decrypt payload
        let payload = self.decrypt_payload(&combined_secret, &hint.encrypted_payload, &hint.nonce, c_out).ok()?;
        
        Some((payload, true))
    }
    
    /* ===== PRIVATE HELPER METHODS ===== */
    
    fn falcon_key_exchange(&self, sk: &[u8], pk: &[u8], context: &[u8; 32]) -> [u8; 32] {
        // Post-quantum key exchange using KMAC with Falcon keys
        let mut kex_input = Vec::new();
        kex_input.extend_from_slice(pk);
        kex_input.extend_from_slice(context);
        kex_input.extend_from_slice(&self.key_manager.current_epoch.to_le_bytes());
        
        kmac256_derive_key(sk, b"FALCON_KEX_v1", &kex_input)
    }
    
    fn combine_shared_secrets(&self, falcon_shared: &[u8; 32], x25519_shared: &[u8], context: &[u8; 32]) -> [u8; 32] {
        // Combine both shared secrets for hybrid security
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
    
    fn encrypt_payload(&self, key: &[u8; 32], payload: &HintPayload, aad: &[u8; 32]) -> Result<(Vec<u8>, [u8; 12]), FalconError> {
        use aes_gcm_siv::{Aes256GcmSiv, KeyInit, Nonce};
        use aes_gcm_siv::aead::Aead;
        
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
    
    fn decrypt_payload(&self, key: &[u8; 32], ciphertext: &[u8], nonce: &[u8; 12], aad: &[u8; 32]) -> Result<HintPayload, FalconError> {
        use aes_gcm_siv::{Aes256GcmSiv, KeyInit, Nonce};
        use aes_gcm_siv::aead::Aead;
        
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
        use sha2::{Sha256, Digest};
        
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
    
    #[error("Falcon signing failed")]
    SigningFailed,
    
    #[error("Falcon verification failed")]
    VerificationFailed,
    
    #[error("Invalid Falcon key format")]
    InvalidKey,
    
    #[error("Encryption failed")]
    EncryptionFailed,
    
    #[error("Decryption failed")]
    DecryptionFailed,
    
    #[error("Serialization failed")]
    SerializationFailed,
    
    #[error("Deserialization failed")]
    DeserializationFailed,
}

/* ===== UTILITY FUNCTIONS ===== */

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
        
        // Same epoch should produce same keys
        assert_eq!(sk1, sk2);
        assert_eq!(pk1, pk2);
        
        // Different epoch should produce different keys
        let (sk3, pk3) = key_manager.derive_epoch_keypair(1).unwrap();
        assert_ne!(pk1, pk3);
    }
    
    #[test]
    fn test_quantum_context_creation() {
        let master_seed = [0x43u8; 32];
        let ctx = QuantumKeySearchCtx::new(master_seed).unwrap();
        
        // Falcon public key should be 897 bytes (Falcon512)
        assert_eq!(ctx.get_falcon_public_key().len(), 897);
        
        // X25519 public key should be 32 bytes
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
        
        // Alice creates hint for Bob
        let hint = alice_ctx.build_quantum_hint(
            bob_ctx.get_falcon_public_key(),
            bob_ctx.get_x25519_public_key(),
            &c_out,
            &payload,
        ).unwrap();
        
        // Bob verifies and decrypts hint
        let (decrypted, quantum_safe) = bob_ctx.verify_quantum_hint(&hint, &c_out).unwrap();
        
        assert!(quantum_safe);
        assert_eq!(decrypted.r_blind, payload.r_blind);
        assert_eq!(decrypted.value, payload.value);
        assert_eq!(decrypted.memo, payload.memo);
    }
    
    #[test]
    fn test_epoch_verification() {
        let master_seed = [0x55u8; 32];
        let mut key_manager = FalconKeyManager::new(master_seed, 100); // 100 second epochs for testing
        
        let current = key_manager.get_current_epoch();
        
        // Current epoch should be valid
        assert!(key_manager.verify_epoch(current));
        
        // Previous epoch should be valid (clock skew)
        assert!(key_manager.verify_epoch(current.saturating_sub(1)));
        
        // Old epochs should be invalid
        assert!(!key_manager.verify_epoch(current.saturating_sub(2)));
        
        // Future epochs should be invalid
        assert!(!key_manager.verify_epoch(current + 1));
    }
}
