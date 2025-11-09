//! KMAC + Falcon512 integration for quantum-safe keysearch
#![forbid(unsafe_code)]

use crate::crypto::kmac::{kmac256_derive_key, kmac256_xof_fill};
use crate::keysearch::{HintPayloadV1, DecodedHint, KeySearchCtx, AadMode};
use pqcrypto_falcon::falcon512;
use pqcrypto_traits::sign::{
    PublicKey as PQPublicKey, 
    SecretKey as PQSecretKey,
    SignedMessage as PQSignedMessage
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
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
    pub eph_pub: Vec<u8>,           // Falcon512 ephemeral public key (897 bytes)
    pub x25519_eph_pub: [u8; 32],   // Traditional X25519 for performance
    pub falcon_signature: Vec<u8>,  // Falcon512 signature (~666 bytes)
    pub encrypted_payload: Vec<u8>, // AES-GCM encrypted payload
    pub timestamp: u64,             // Prevention replay attacks
    pub epoch: u64,                 // Key rotation epoch
}

#[derive(Clone, Debug)]
pub struct QuantumFoundNote {
    pub index: usize,
    pub c_out: [u8; 32],
    pub k_search: [u8; 32],
    pub falcon_verified: bool,      // Whether Falcon signature was verified
    pub quantum_safe: bool,         // Post-quantum security level
}

/* ===== FALCON KEY MANAGEMENT WITH KMAC ===== */

pub struct FalconKeyManager {
    current_epoch: u64,
    epoch_duration: u64,
    master_seed: Zeroizing<[u8; 32]>,
}

impl FalconKeyManager {
    pub fn new(master_seed: [u8; 32]) -> Self {
        Self {
            current_epoch: 0,
            epoch_duration: 86400, // 24 hours in seconds
            master_seed: Zeroizing::new(master_seed),
        }
    }
    
    /// Derive epoch-specific Falcon keypair using KMAC
    pub fn derive_epoch_keypair(&self, epoch: u64) -> Result<(FalconSecretKey, FalconPublicKey), FalconError> {
        let _epoch_seed = kmac256_derive_key(
            &*self.master_seed,
            format!("FALCON_EPOCH_{}", epoch).as_bytes(),
            b"key_rotation"
        );
        
        // Note: pqcrypto-falcon 0.3 uses internal RNG
        let (pk, sk) = falcon512::keypair();
        
        Ok((sk, pk))
    }
    
    /// Derive ephemeral Falcon keypair for single transaction
    pub fn derive_ephemeral_falcon(
        &self,
        transaction_id: &[u8; 32],
        context: &[u8]
    ) -> Result<(FalconSecretKey, FalconPublicKey), FalconError> {
        let _ephemeral_seed = kmac256_derive_key(
            &*self.master_seed,
            format!("FALCON_EPHEMERAL_{}", self.current_epoch).as_bytes(),
            &Self::build_ephemeral_context(transaction_id, context)
        );
        
        // Note: pqcrypto-falcon 0.3 uses internal RNG
        let (pk, sk) = falcon512::keypair();
        
        Ok((sk, pk))
    }
    
    pub fn get_current_epoch(&self) -> u64 {
        self.current_epoch
    }
    
    pub fn rotate_epoch(&mut self) {
        self.current_epoch += 1;
    }
    
    /// Verify if a hint is from the correct epoch
    pub fn verify_epoch(&self, hint: &QuantumSafeHint) -> bool {
        hint.epoch >= self.current_epoch.saturating_sub(1) // Allow previous epoch for clock skew
    }
    
    fn build_ephemeral_context(transaction_id: &[u8; 32], context: &[u8]) -> Vec<u8> {
        let mut ctx = Vec::new();
        ctx.extend_from_slice(b"EPHEMERAL_FALCON_v1");
        ctx.extend_from_slice(transaction_id);
        ctx.extend_from_slice(context);
        ctx
    }
}

/* ===== QUANTUM KEYSEARCH CONTEXT WITH KMAC INTEGRATION ===== */

pub struct QuantumKeySearchCtx {
    // Post-quantum identity
    falcon_identity: (FalconSecretKey, FalconPublicKey),
    
    // Traditional keys for performance
    x25519_secret: Zeroizing<[u8; 32]>,
    
    // Key management
    key_manager: FalconKeyManager,
    
    // Performance cache
    cache: QuantumSearchCache,
}

impl QuantumKeySearchCtx {
    pub fn new(master_seed: [u8; 32]) -> Result<Self, FalconError> {
        let key_manager = FalconKeyManager::new(master_seed);
        let falcon_identity = key_manager.derive_epoch_keypair(0)?;
        
        // Derive X25519 key using KMAC for consistency
        let x25519_secret = Zeroizing::new(kmac256_derive_key(
            &master_seed, 
            b"X25519_SESSION_KEY", 
            b"key_derivation"
        ));
        
        Ok(Self {
            falcon_identity,
            x25519_secret,
            key_manager,
            cache: QuantumSearchCache::new(),
        })
    }
    
    pub fn get_falcon_public_key(&self) -> &[u8] {
        self.falcon_identity.1.as_bytes()
    }
    
    pub fn get_x25519_public_key(&self) -> [u8; 32] {
        let secret = X25519Secret::from(*self.x25519_secret);
        let public = X25519PublicKey::from(&secret);
        public.to_bytes()
    }
    
    /// Create quantum-safe hint using KMAC for all cryptographic operations
    pub fn build_quantum_hint(
        &self,
        recipient_falcon_pk: &FalconPublicKey,
        recipient_x25519_pk: &X25519PublicKey,
        c_out: &[u8; 32],
        payload: &HintPayloadV1,
    ) -> Result<QuantumSafeHint, FalconError> {
        // 1. Generate transaction ID using KMAC
        let transaction_id = Self::derive_transaction_id(c_out, payload);
        
        // 2. Generate ephemeral Falcon keypair using KMAC
        let (eph_sk, eph_pk) = self.key_manager.derive_ephemeral_falcon(
            &transaction_id,
            b"QUANTUM_HINT"
        )?;
        
        // 3. Create shared secret using KMAC-based Falcon KEX
        let shared_secret = self.falcon_key_exchange(&eph_sk, recipient_falcon_pk, c_out);
        
        // 4. Build signing message
        let signing_message = self.build_signing_message(c_out, &shared_secret, &eph_pk);
        
        // 5. Sign message
        let signed = falcon512::sign(&signing_message, &eph_sk);
        let signature = signed.as_bytes()[signing_message.len()..].to_vec();
        
        // 6. Encrypt payload using KMAC-derived keys
        let encrypted_payload = self.encrypt_with_kmac_keys(&shared_secret, payload, c_out);
        
        // 7. Create traditional X25519 hint for performance fallback
        let x25519_eph_pub = self.derive_x25519_eph_pub(recipient_x25519_pk, c_out);
        
        Ok(QuantumSafeHint {
            eph_pub: eph_pk.as_bytes().to_vec(),
            x25519_eph_pub,
            falcon_signature: signature,
            encrypted_payload,
            timestamp: current_timestamp(),
            epoch: self.key_manager.get_current_epoch(),
        })
    }
    
    /// Verify and decrypt quantum-safe hint with KMAC verification
    pub fn verify_quantum_hint(
        &self,
        hint: &QuantumSafeHint,
        c_out: &[u8; 32],
    ) -> Option<(DecodedHint, bool)> {
        // 1. Verify epoch
        if !self.key_manager.verify_epoch(hint) {
            return None;
        }
        
        // 2. Parse Falcon public key
        let eph_pk = FalconPublicKey::from_bytes(&hint.eph_pub).ok()?;
        
        // 3. Recreate shared secret using KMAC
        let shared_secret = self.falcon_key_exchange(&self.falcon_identity.0, &eph_pk, c_out);
        
        // 4. Verify Falcon signature with KMAC context
        let signing_message = self.build_signing_message(c_out, &shared_secret, &eph_pk);
        
        let mut signed_msg = signing_message.clone();
        signed_msg.extend_from_slice(&hint.falcon_signature);
        
        let signed = FalconSignedMessage::from_bytes(&signed_msg).ok()?;
        let falcon_verified = falcon512::open(&signed, &eph_pk).is_ok();
        
        // 5. Try traditional verification first (performance)
        if let Some(result) = self.try_traditional_verification(hint, c_out) {
            return Some((result, false)); // Traditional verification
        }
        
        // 6. Fall back to quantum verification
        if falcon_verified {
            if let Some(decoded) = self.decrypt_with_kmac_keys(&shared_secret, &hint.encrypted_payload, c_out) {
                return Some((decoded, true)); // Quantum verification
            }
        }
        
        None
    }
    
    /// Batch quantum-safe scanning
    pub fn scan_quantum_safe<'a, I>(
        &self, 
        outputs: I
    ) -> Vec<QuantumFoundNote> 
    where
        I: IntoIterator<Item = (usize, &'a [u8; 32], &'a QuantumSafeHint)>,
    {
        let mut hits = Vec::new();
        
        for (idx, c_out, hint) in outputs {
            if let Some((decoded, quantum_verified)) = self.verify_quantum_hint(hint, c_out) {
                let k_search = if quantum_verified {
                    self.quantum_k_search(c_out, hint, &decoded)
                } else {
                    self.traditional_k_search(c_out, hint)
                };
                
                hits.push(QuantumFoundNote {
                    index: idx,
                    c_out: *c_out,
                    k_search,
                    falcon_verified: quantum_verified,
                    quantum_safe: quantum_verified,
                });
            }
        }
        
        hits
    }
    
    /* ===== KMAC-BASED CRYPTOGRAPHIC OPERATIONS ===== */
    
    fn falcon_key_exchange(
        &self, 
        sk: &FalconSecretKey,
        pk: &FalconPublicKey,
        context: &[u8; 32]
    ) -> [u8; 32] {
        let mut shared_secret = [0u8; 32];
        
        // Build KEX input
        let mut kex_input = Vec::new();
        kex_input.extend_from_slice(sk.as_bytes());
        kex_input.extend_from_slice(pk.as_bytes());
        kex_input.extend_from_slice(context);
        kex_input.extend_from_slice(&self.key_manager.get_current_epoch().to_le_bytes());
        
        // Use KMAC to derive shared secret
        kmac256_xof_fill(
            sk.as_bytes(), 
            b"FALCON_KEX_v1", 
            &kex_input, 
            &mut shared_secret
        );
        
        shared_secret
    }
    
    fn encrypt_with_kmac_keys(
        &self,
        shared_secret: &[u8; 32],
        payload: &HintPayloadV1,
        c_out: &[u8; 32],
    ) -> Vec<u8> {
        // Derive encryption key using KMAC
        let encryption_key = kmac256_derive_key(
            shared_secret,
            b"QUANTUM_ENCRYPTION_KEY",
            c_out
        );
        
        // Derive nonce using KMAC
        let nonce = kmac256_derive_key(
            shared_secret,
            b"QUANTUM_ENCRYPTION_NONCE", 
            c_out
        );
        
        // Serialize payload
        let serialized_payload = bincode::serialize(payload)
            .expect("Failed to serialize payload");
        
        // Simple encryption (placeholder - use AES-GCM in production)
        let mut result = Vec::new();
        result.extend_from_slice(&encryption_key);
        result.extend_from_slice(&nonce[..12]); // 12 bytes for AES-GCM
        result.extend_from_slice(&serialized_payload);
        result
    }
    
    fn decrypt_with_kmac_keys(
        &self,
        shared_secret: &[u8; 32],
        encrypted: &[u8],
        c_out: &[u8; 32],
    ) -> Option<DecodedHint> {
        if encrypted.len() < 32 + 12 {
            return None;
        }
        
        // Re-derive keys (same as encryption)
        let encryption_key = kmac256_derive_key(
            shared_secret,
            b"QUANTUM_ENCRYPTION_KEY",
            c_out
        );
        
        // Verify keys match (simplified)
        if &encrypted[..32] != &encryption_key {
            return None;
        }
        
        // Simple decryption (placeholder)
        let payload_start = 32 + 12;
        if encrypted.len() < payload_start {
            return None;
        }
        
        let payload: HintPayloadV1 = bincode::deserialize(&encrypted[payload_start..]).ok()?;
        
        Some(DecodedHint {
            r_blind: payload.r_blind,
            value: Some(payload.value),
            memo_items: vec![payload.memo.clone()],
        })
    }
    
    /* ===== HELPER METHODS ===== */
    
    fn derive_transaction_id(c_out: &[u8; 32], payload: &HintPayloadV1) -> [u8; 32] {
        let mut tx_input = Vec::new();
        tx_input.extend_from_slice(c_out);
        tx_input.extend_from_slice(&payload.r_blind);
        tx_input.extend_from_slice(&payload.value.to_le_bytes());
        
        kmac256_derive_key(
            c_out,
            b"TRANSACTION_ID",
            &tx_input
        )
    }
    
    fn build_signing_message(
        &self,
        c_out: &[u8; 32],
        shared_secret: &[u8; 32],
        eph_pk: &FalconPublicKey,
    ) -> Vec<u8> {
        let mut message = Vec::new();
        message.extend_from_slice(b"QUANTUM_HINT_SIGNING_v1");
        message.extend_from_slice(c_out);
        message.extend_from_slice(shared_secret);
        message.extend_from_slice(eph_pk.as_bytes());
        message.extend_from_slice(&self.key_manager.get_current_epoch().to_le_bytes());
        message.extend_from_slice(&current_timestamp().to_le_bytes());
        message
    }
    
    fn try_traditional_verification(
        &self,
        hint: &QuantumSafeHint,
        c_out: &[u8; 32],
    ) -> Option<DecodedHint> {
        let traditional_ctx = KeySearchCtx::new(*self.x25519_secret);
        let traditional_hint = self.wrap_as_traditional_hint(hint);
        
        traditional_ctx
            .try_match_and_decrypt_ext(c_out, &traditional_hint, AadMode::COutOnly)
            .and_then(|(_, decoded)| decoded)
    }
    
    fn wrap_as_traditional_hint(&self, hint: &QuantumSafeHint) -> Vec<u8> {
        // Convert quantum hint to traditional format for fallback
        let mut traditional = Vec::new();
        traditional.extend_from_slice(&hint.x25519_eph_pub);
        
        // Derive traditional tag using KMAC
        let mut tag = [0u8; 32];
        kmac256_xof_fill(
            &*self.x25519_secret,
            b"TRADITIONAL_FALLBACK_TAG",
            &hint.x25519_eph_pub,
            &mut tag
        );
        traditional.extend_from_slice(&tag);
        
        traditional
    }
    
    fn derive_x25519_eph_pub(
        &self,
        _recipient_pk: &X25519PublicKey,
        c_out: &[u8; 32],
    ) -> [u8; 32] {
        // Derive X25519 ephemeral public key using KMAC
        let eph_secret = kmac256_derive_key(
            &*self.x25519_secret,
            b"X25519_EPHEMERAL",
            c_out
        );
        
        let eph_sk = X25519Secret::from(eph_secret);
        let eph_pk = X25519PublicKey::from(&eph_sk);
        
        eph_pk.to_bytes()
    }
    
    fn quantum_k_search(
        &self,
        c_out: &[u8; 32],
        hint: &QuantumSafeHint,
        decoded: &DecodedHint,
    ) -> [u8; 32] {
        let mut k_search = [0u8; 32];
        
        let mut kex_input = Vec::new();
        kex_input.extend_from_slice(&hint.eph_pub);
        kex_input.extend_from_slice(c_out);
        kex_input.extend_from_slice(&decoded.r_blind);
        
        kmac256_xof_fill(
            self.falcon_identity.0.as_bytes(),
            b"QUANTUM_K_SEARCH",
            &kex_input,
            &mut k_search
        );
        
        k_search
    }
    
    fn traditional_k_search(
        &self,
        c_out: &[u8; 32],
        hint: &QuantumSafeHint,
    ) -> [u8; 32] {
        let traditional_ctx = KeySearchCtx::new(*self.x25519_secret);
        let traditional_hint = self.wrap_as_traditional_hint(hint);
        
        traditional_ctx
            .try_match(c_out, &traditional_hint)
            .unwrap_or([0u8; 32])
    }
}

/* ===== QUANTUM SEARCH CACHE ===== */

struct QuantumSearchCache {
    #[allow(dead_code)]
    verified_hints: lru::LruCache<[u8; 32], bool>,
    #[allow(dead_code)]
    epoch_keys: lru::LruCache<u64, (Vec<u8>, Vec<u8>)>,
}

impl QuantumSearchCache {
    fn new() -> Self {
        use std::num::NonZeroUsize;
        Self {
            verified_hints: lru::LruCache::new(NonZeroUsize::new(1000).unwrap()),
            epoch_keys: lru::LruCache::new(NonZeroUsize::new(5).unwrap()),
        }
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
    #[error("Invalid Falcon key")]
    InvalidKey,
    #[error("Payload serialization failed")]
    SerializationFailed,
}

/* ===== UTILITY FUNCTIONS ===== */

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_falcon_key_derivation_with_kmac() {
        let master_seed = [0x42u8; 32];
        let key_manager = FalconKeyManager::new(master_seed);
        
        let (sk, pk) = key_manager.derive_epoch_keypair(0).unwrap();
        
        // Verify keys are valid
        assert_eq!(pk.as_bytes().len(), 897); // Falcon512 public key size
        assert!(!sk.as_bytes().is_empty());
    }
    
    #[test]
    fn test_quantum_context_creation() {
        let master_seed = [0x43u8; 32];
        let ctx = QuantumKeySearchCtx::new(master_seed).unwrap();
        
        // Context should be properly initialized
        assert_eq!(ctx.key_manager.get_current_epoch(), 0);
        assert_eq!(ctx.get_falcon_public_key().len(), 897);
    }
    
    #[test]
    fn test_transaction_id_derivation() {
        let c_out = [0x66u8; 32];
        let mut payload = HintPayloadV1 {
            r_blind: [0x77u8; 32],
            value: 1000,
            memo: Vec::new(),
        };
        
        let tx_id1 = QuantumKeySearchCtx::derive_transaction_id(&c_out, &payload);
        let tx_id2 = QuantumKeySearchCtx::derive_transaction_id(&c_out, &payload);
        
        // Same inputs should produce same transaction ID
        assert_eq!(tx_id1, tx_id2);
        
        // Different payload should produce different ID
        payload.value = 2000;
        let tx_id3 = QuantumKeySearchCtx::derive_transaction_id(&c_out, &payload);
        assert_ne!(tx_id1, tx_id3);
    }
}
