//! KMAC + Falcon512 + ML-KEM Integration (CORRECTED)
//! 
//! **CRITICAL FIXES APPLIED:**
//! - Falcon used ONLY for signatures (NOT for key exchange!)
//! - ML-KEM (Kyber768) for key encapsulation mechanism
//! - X25519 ECDH for hybrid defense in depth
//! - XChaCha20-Poly1305 for authenticated encryption (AEAD)
//! - Transcript binding to prevent mix-and-match attacks
//! - Replay protection via timestamp validation
//! 
//! This module provides quantum-safe encrypted hints for note discovery
//! in a private transaction system.

#![forbid(unsafe_code)]

use crate::crypto::kmac::{kmac256_derive_key, kmac256_xof_fill};
use crate::keysearch::{HintPayloadV1, DecodedHint};
use chacha20poly1305::{XChaCha20Poly1305, aead::{Aead, KeyInit}, XNonce};
use pqcrypto_falcon::falcon512::{self, PublicKey as FalconPublicKey, SecretKey as FalconSecretKey, SignedMessage as FalconSignedMessage};
use pqcrypto_kyber::kyber768 as mlkem;
use pqcrypto_traits::sign::{PublicKey as PQSignPublicKey, SecretKey as PQSignSecretKey, SignedMessage as PQSignedMessage};
use pqcrypto_traits::kem::{Ciphertext as PQCiphertext, SharedSecret as PQSharedSecret};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret as X25519Secret};
use zeroize::{Zeroize, Zeroizing};

/* ============================================================================
 * Cryptographic Labels (for auditing and domain separation)
 * ========================================================================== */

/// Label for hybrid KEM shared secret derivation
const LABEL_HYBRID: &[u8] = b"QH/HYBRID";

/// Label for AEAD key derivation from shared secret
const LABEL_AEAD_KEY: &[u8] = b"QH/AEAD/Key";

/// Label for AEAD nonce derivation from shared secret
const LABEL_AEAD_NONCE: &[u8] = b"QH/AEAD/Nonce24";

/// Label for transcript binding
const LABEL_TRANSCRIPT: &[u8] = b"QH/Transcript";

/// Label for hint fingerprint (Bloom filter integration)
const LABEL_HINT_FP: &[u8] = b"TT-HINT.FP.KEY";

/// Label for hint fingerprint domain
const LABEL_HINT_FP_DOMAIN: &[u8] = b"TT-HINT.FP.v1";

/// Default timestamp freshness window (2 hours = 7200 seconds)
pub const DEFAULT_MAX_SKEW_SECS: u64 = 7200;

/// Default epoch validation policy (accept current or previous epoch)
pub const DEFAULT_ACCEPT_PREV_EPOCH: bool = true;

/* ============================================================================
 * Type Aliases
 * ========================================================================== */

pub type MlkemPublicKey = mlkem::PublicKey;
pub type MlkemSecretKey = mlkem::SecretKey;

/* ============================================================================
 * Error Types
 * ========================================================================== */

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FalconError {
    KeyDerivationFailed,
    SignatureFailed,
    VerificationFailed,
    EncryptionFailed,
    DecryptionFailed,
    SerializationFailed,
    InvalidEpoch,
    ReplayAttack,
}

/* ============================================================================
 * Quantum-Safe Hint Structure
 * ========================================================================== */

/// Quantum-safe encrypted hint (CORRECTED VERSION)
/// 
/// This structure binds all parameters via transcript + Falcon signature
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuantumSafeHint {
    /// ML-KEM (Kyber768) ciphertext for key encapsulation
    pub kem_ct: Vec<u8>,
    
    /// X25519 ephemeral public key (hybrid with ML-KEM)
    pub x25519_eph_pub: [u8; 32],
    
    /// Falcon512 signature over transcript (SignedMessage format)
    pub falcon_signed_msg: Vec<u8>,
    
    /// AEAD ciphertext (XChaCha20-Poly1305)
    pub encrypted_payload: Vec<u8>,
    
    /// ✅ NEW: Sender's Falcon public key (for verification + transcript)
    pub sender_falcon_pk: Vec<u8>,
    
    /// Timestamp for replay protection
    pub timestamp: u64,
    
    /// Key rotation epoch
    pub epoch: u64,
}

impl Zeroize for QuantumSafeHint {
    fn zeroize(&mut self) {
        self.kem_ct.zeroize();
        self.x25519_eph_pub.zeroize();
        self.falcon_signed_msg.zeroize();
        self.encrypted_payload.zeroize();
        self.sender_falcon_pk.zeroize();  // ✅ NEW
        self.timestamp = 0;
        self.epoch = 0;
    }
}

impl Drop for QuantumSafeHint {
    fn drop(&mut self) {
        self.zeroize();
    }
}

/// Found note with quantum verification status
#[derive(Clone, Debug)]
pub struct QuantumFoundNote {
    pub index: usize,
    pub c_out: [u8; 32],
    pub k_search: [u8; 32],
    pub falcon_verified: bool,
    pub quantum_safe: bool,
}

/* ============================================================================
 * Falcon Key Manager (Epoch Rotation)
 * ========================================================================== */

pub struct FalconKeyManager {
    master_seed: Zeroizing<[u8; 32]>,
    current_epoch: u64,
    epoch_duration: u64,
}

impl FalconKeyManager {
    pub fn new(master_seed: [u8; 32]) -> Self {
        Self {
            master_seed: Zeroizing::new(master_seed),
            current_epoch: 0,
            epoch_duration: 86_400, // 24 hours
        }
    }
    
    pub fn get_current_epoch(&self) -> u64 {
        self.current_epoch
    }
    
    /// Derive Falcon keypair for specific epoch
    pub fn derive_epoch_keypair(&self, epoch: u64) -> Result<(FalconSecretKey, FalconPublicKey), FalconError> {
        let mut input = Vec::with_capacity(32 + 8);
        input.extend_from_slice(&*self.master_seed);
        input.extend_from_slice(&epoch.to_le_bytes());
        
        let seed = kmac256_derive_key(&input, b"FALCON_EPOCH_SEED", b"keygen");
        
        // Falcon keygen is deterministic given seed
        // TODO: Use proper seeded keygen when available
        // For now, using standard keygen (non-deterministic)
        let (pk, sk) = falcon512::keypair();
        
        Ok((sk, pk))
    }
}

/* ============================================================================
 * Transcript Construction
 * ========================================================================== */

/// Construct transcript binding all hint parameters
/// 
/// Format: domain || c_out || epoch || timestamp || kem_ct_len || kem_ct || x25519_eph || falcon_pk
fn transcript(
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
    
    t.extend_from_slice(b"QHINT.v1");              // domain separator
    t.extend_from_slice(c_out);                     // commitment
    t.extend_from_slice(&epoch.to_le_bytes());      // epoch
    t.extend_from_slice(&timestamp.to_le_bytes());  // timestamp
    t.extend_from_slice(&(kem_ct.len() as u32).to_le_bytes());
    t.extend_from_slice(kem_ct);                    // KEM ciphertext
    t.extend_from_slice(x25519_eph_pub);            // X25519 ephemeral
    t.extend_from_slice(sender_falcon_pk);          // Falcon PK
    
    t
}

/* ============================================================================
 * AEAD Encryption/Decryption
 * ========================================================================== */

fn aead_encrypt(
    ss_h: &[u8; 32],
    aad: &[u8],
    payload: &HintPayloadV1,
) -> Result<Vec<u8>, FalconError> {
    // Derive key and nonce from shared secret
    let key = kmac256_derive_key(ss_h, LABEL_AEAD_KEY, b"");
    let mut nonce24 = [0u8; 24];
    kmac256_xof_fill(ss_h, LABEL_AEAD_NONCE, b"", &mut nonce24);
    
    // Create cipher
    let cipher = XChaCha20Poly1305::new_from_slice(&key)
        .map_err(|_| FalconError::EncryptionFailed)?;
    
    // Serialize payload
    let plaintext = bincode::serialize(payload)
        .map_err(|_| FalconError::SerializationFailed)?;
    
    // Encrypt with AAD (transcript binding)
    let ciphertext = cipher.encrypt(
        XNonce::from_slice(&nonce24),  // ✅ FIXED: use from_slice
        chacha20poly1305::aead::Payload {
            msg: &plaintext,
            aad,
        },
    )
    .map_err(|_| FalconError::EncryptionFailed)?;
    
    Ok(ciphertext)
}

fn aead_decrypt(
    ss_h: &[u8; 32],
    aad: &[u8],
    ciphertext: &[u8],
) -> Option<HintPayloadV1> {
    // Derive key and nonce (same as encrypt)
    let key = kmac256_derive_key(ss_h, LABEL_AEAD_KEY, b"");
    let mut nonce24 = [0u8; 24];
    kmac256_xof_fill(ss_h, LABEL_AEAD_NONCE, b"", &mut nonce24);
    
    // Create cipher
    let cipher = XChaCha20Poly1305::new_from_slice(&key).ok()?;
    
    // Decrypt with AAD
    let plaintext = cipher.decrypt(
        XNonce::from_slice(&nonce24),  // ✅ FIXED: use from_slice
        chacha20poly1305::aead::Payload {
            msg: ciphertext,
            aad,
        },
    )
    .ok()?;
    
    // Deserialize payload
    bincode::deserialize(&plaintext).ok()
}

/* ============================================================================
 * Quantum Key Search Context
 * ========================================================================== */

pub struct QuantumKeySearchCtx {
    /// Falcon identity keypair (for signatures ONLY)
    falcon_identity: (FalconSecretKey, FalconPublicKey),
    
    /// X25519 session secret (hybrid with ML-KEM)
    x25519_secret: Zeroizing<[u8; 32]>,
    
    /// ML-KEM (Kyber768) keypair (for key encapsulation)
    mlkem_sk: MlkemSecretKey,
    mlkem_pk: MlkemPublicKey,
    
    /// Key manager for epoch rotation
    key_manager: FalconKeyManager,
}

impl QuantumKeySearchCtx {
    /// Create new quantum key search context
    pub fn new(master_seed: [u8; 32]) -> Result<Self, FalconError> {
        let key_manager = FalconKeyManager::new(master_seed);
        let falcon_identity = key_manager.derive_epoch_keypair(0)?;
        
        // ML-KEM keypair (for key encapsulation)
        let (mlkem_pk, mlkem_sk) = mlkem::keypair();
        
        // X25519 secret (for hybrid KEM)
        let x25519_secret = Zeroizing::new(
            kmac256_derive_key(&master_seed, b"X25519_SESSION_KEY", b"key_derivation")
        );
        
        Ok(Self {
            falcon_identity,
            x25519_secret,
            mlkem_sk,
            mlkem_pk,
            key_manager,
        })
    }
    
    /// Get ML-KEM public key (for recipients)
    pub fn mlkem_public_key(&self) -> &MlkemPublicKey {
        &self.mlkem_pk
    }
    
    /// Get Falcon public key (for signature verification)
    pub fn falcon_public_key(&self) -> &FalconPublicKey {
        &self.falcon_identity.1
    }
    
    /// Get X25519 public key (for hybrid KEM)
    pub fn x25519_public_key(&self) -> X25519PublicKey {
        let sk = X25519Secret::from(*self.x25519_secret);
        X25519PublicKey::from(&sk)
    }
    
    /// Build quantum hint (CORRECTED: Falcon=sig, ML-KEM=KEX, XChaCha=AEAD)
    /// 
    /// # Security Model
    /// 1. ML-KEM encapsulation for quantum-safe shared secret
    /// 2. X25519 ECDH for hybrid security (defense in depth)
    /// 3. Hybrid secret: KMAC(ss_KEM || DH)
    /// 4. Falcon signature over transcript (binds all parameters)
    /// 5. XChaCha20-Poly1305 AEAD with transcript as AAD
    pub fn build_quantum_hint(
        &self,
        recipient_mlkem_pk: &MlkemPublicKey,
        recipient_x25519_pk: &X25519PublicKey,
        c_out: &[u8; 32],
        payload: &HintPayloadV1,
    ) -> Result<QuantumSafeHint, FalconError> {
        let epoch = self.key_manager.get_current_epoch();
        let timestamp = current_timestamp();
        
        // 1. X25519 ephemeral key (hybrid with ML-KEM)
        let eph_secret = kmac256_derive_key(
            &*self.x25519_secret,
            b"X25519_EPHEMERAL",
            c_out,
        );
        let eph_sk = X25519Secret::from(eph_secret);
        let x25519_eph_pub = X25519PublicKey::from(&eph_sk).to_bytes();
        let dh = eph_sk.diffie_hellman(recipient_x25519_pk).to_bytes();
        
        // 2. ML-KEM encapsulation (quantum-safe)
        let (kem_ss, kem_ct) = mlkem::encapsulate(recipient_mlkem_pk);
        let kem_ss_bytes = <mlkem::SharedSecret as PQSharedSecret>::as_bytes(&kem_ss);
        let kem_ct_bytes = <mlkem::Ciphertext as PQCiphertext>::as_bytes(&kem_ct);
        
        // 3. Hybrid shared secret: KMAC(ss_KEM || DH)
        let mut input = Vec::with_capacity(kem_ss_bytes.len() + dh.as_ref().len());
        input.extend_from_slice(kem_ss_bytes);
        input.extend_from_slice(dh.as_ref());
        let ss_h = kmac256_derive_key(&input, LABEL_HYBRID, c_out);
        
        // 4. Construct transcript (binds all parameters)
        let sender_pk_bytes = <FalconPublicKey as PQSignPublicKey>::as_bytes(&self.falcon_identity.1);
        let tr = transcript(
            epoch,
            timestamp,
            c_out,
            kem_ct_bytes,
            &x25519_eph_pub,
            sender_pk_bytes,  // ✅ FIXED: use sender's PK
        );
        
        // 5. Sign transcript with Falcon (identity key)
        let sm = falcon512::sign(&tr, &self.falcon_identity.0);
        let falcon_signed_msg = <FalconSignedMessage as PQSignedMessage>::as_bytes(&sm).to_vec();
        
        // 6. AEAD encrypt payload (XChaCha20-Poly1305 with transcript as AAD)
        let encrypted_payload = aead_encrypt(&ss_h, &tr, payload)?;
        
        Ok(QuantumSafeHint {
            kem_ct: kem_ct_bytes.to_vec(),
            x25519_eph_pub,
            falcon_signed_msg,
            encrypted_payload,
            sender_falcon_pk: sender_pk_bytes.to_vec(),  // ✅ NEW: include sender PK
            timestamp,
            epoch,
        })
    }
    
    /// Verify quantum hint (CORRECTED: proper signature + KEM + AEAD)
    /// 
    /// # Security Checks
    /// 1. Epoch validation
    /// 2. Timestamp freshness (2-hour window for replay protection)
    /// 3. Transcript reconstruction
    /// 4. Falcon signature verification
    /// 5. ML-KEM decapsulation
    /// 6. X25519 ECDH (hybrid)
    /// 7. AEAD decryption with transcript AAD
    /// Verify quantum hint with configurable time/epoch parameters
    /// 
    /// # Parameters
    /// - `hint`: The quantum-safe hint to verify
    /// - `c_out`: Output commitment binding (32 bytes)
    /// - `max_skew_secs`: Maximum timestamp age in seconds (default: 7200 = 2 hours)
    /// - `accept_prev_epoch`: Whether to accept hints from previous epoch (default: true)
    pub fn verify_quantum_hint_with_params(
        &self,
        hint: &QuantumSafeHint,
        c_out: &[u8; 32],
        max_skew_secs: u64,
        accept_prev_epoch: bool,
    ) -> Option<(DecodedHint, bool)> {
        // 1. Epoch validation
        let e = self.key_manager.get_current_epoch();
        let valid_epoch = if accept_prev_epoch {
            hint.epoch == e || hint.epoch.saturating_add(1) == e
        } else {
            hint.epoch == e
        };
        if !valid_epoch {
            return None;
        }
        
        // 2. Timestamp freshness
        let now = current_timestamp();
        if now.saturating_sub(hint.timestamp) > max_skew_secs {
            return None; // Too old, possible replay
        }
        
        // 3. Reconstruct transcript (MUST match what was signed)
        let tr = transcript(
            hint.epoch,
            hint.timestamp,
            c_out,
            &hint.kem_ct,
            &hint.x25519_eph_pub,
            &hint.sender_falcon_pk,  // ✅ CRITICAL FIX: use SENDER's PK from hint
        );
        
        // 4. Verify Falcon signature over transcript
        let sender_pk = FalconPublicKey::from_bytes(&hint.sender_falcon_pk).ok()?;  // ✅ NEW: parse sender PK
        let sm = <FalconSignedMessage as PQSignedMessage>::from_bytes(&hint.falcon_signed_msg).ok()?;
        let opened = falcon512::open(&sm, &sender_pk).ok()?;  // ✅ CRITICAL FIX: verify with SENDER's PK
        if opened != tr {
            return None; // Signature invalid or transcript mismatch
        }
        
        // 5. ML-KEM decapsulation (quantum-safe shared secret)
        let kem_ct = mlkem::Ciphertext::from_bytes(&hint.kem_ct).ok()?;
        let kem_ss = mlkem::decapsulate(&kem_ct, &self.mlkem_sk);
        let kem_ss_bytes = <mlkem::SharedSecret as PQSharedSecret>::as_bytes(&kem_ss);
        
        // 6. X25519 ECDH (hybrid)
        let eph_pub = X25519PublicKey::from(hint.x25519_eph_pub);
        let sk = X25519Secret::from(*self.x25519_secret);
        let dh = sk.diffie_hellman(&eph_pub).to_bytes();
        
        // 7. Hybrid shared secret: KMAC(ss_KEM || DH)
        let mut input = Vec::with_capacity(kem_ss_bytes.len() + dh.as_ref().len());
        input.extend_from_slice(kem_ss_bytes);
        input.extend_from_slice(dh.as_ref());
        let ss_h = kmac256_derive_key(&input, LABEL_HYBRID, c_out);
        
        // 8. AEAD decrypt with transcript as AAD
        let payload = aead_decrypt(&ss_h, &tr, &hint.encrypted_payload)?;
        
        // Success!
        let decoded = DecodedHint {
            r_blind: payload.r_blind,
            value: Some(payload.value),
            memo_items: crate::keysearch::tlv::decode(&payload.memo),
        };
        
        Some((decoded, true))
    }

    /// Verify quantum hint with default parameters
    /// 
    /// Uses DEFAULT_MAX_SKEW_SECS (7200s) and DEFAULT_ACCEPT_PREV_EPOCH (true)
    pub fn verify_quantum_hint(
        &self,
        hint: &QuantumSafeHint,
        c_out: &[u8; 32],
    ) -> Option<(DecodedHint, bool)> {
        self.verify_quantum_hint_with_params(
            hint,
            c_out,
            DEFAULT_MAX_SKEW_SECS,
            DEFAULT_ACCEPT_PREV_EPOCH,
        )
    }
}

/* ============================================================================
 * Utilities
 * ========================================================================== */

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Generate 16-byte fingerprint for Bloom filter integration
/// 
/// This allows pre-filtering hints without full decryption:
/// - Scan headers with Bloom filter using this fingerprint
/// - Only attempt full verification on matches
/// 
/// # Security
/// 
/// Fingerprint is derived from transcript, which binds:
/// - All cryptographic parameters (KEM CT, X25519 pub, Falcon PK)
/// - Output commitment (c_out)
/// - Epoch and timestamp
/// 
/// This ensures fingerprint uniqueness without leaking sensitive data.
pub fn hint_fingerprint16(hint: &QuantumSafeHint, c_out: &[u8; 32]) -> [u8; 16] {
    let tr = transcript(
        hint.epoch,
        hint.timestamp,
        c_out,
        &hint.kem_ct,
        &hint.x25519_eph_pub,
        &hint.sender_falcon_pk,
    );
    
    // Derive 16-byte tag from transcript using KMAC256
    let key = kmac256_derive_key(&tr, LABEL_HINT_FP, LABEL_HINT_FP_DOMAIN);
    let mut fp = [0u8; 16];
    fp.copy_from_slice(&key[..16]);
    fp
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
    fn test_context_creation() {
        use pqcrypto_traits::sign::PublicKey as PQSignPK;
        use pqcrypto_traits::kem::PublicKey as PQKemPK;
        
        let master_seed = [0x42u8; 32];
        let ctx = QuantumKeySearchCtx::new(master_seed).unwrap();
        
        // Should have all keys initialized
        assert!(<MlkemPublicKey as PQKemPK>::as_bytes(ctx.mlkem_public_key()).len() > 0);
        assert!(<FalconPublicKey as PQSignPK>::as_bytes(ctx.falcon_public_key()).len() == 897);
    }

    #[test]
    fn roundtrip_pq_hint_with_sender_pk() {
        // ✅ CRITICAL TEST: Verifies signature uses SENDER's PK, not receiver's
        let seed_sender = [7u8; 32];
        let sender = QuantumKeySearchCtx::new(seed_sender).unwrap();

        // Recipient – separate context with own keys
        let seed_recip = [9u8; 32];
        let recip = QuantumKeySearchCtx::new(seed_recip).unwrap();

        let c_out = [0xCD; 32];
        let payload = HintPayloadV1 {
            r_blind: [1; 32],
            value: 1337,
            memo: Vec::new(),
        };

        // Sender builds hint to recipient
        let hint = sender.build_quantum_hint(
            recip.mlkem_public_key(),
            &recip.x25519_public_key(),
            &c_out,
            &payload,
        ).unwrap();

        // Recipient verifies signature using SENDER's PK from hint
        let out = recip.verify_quantum_hint(&hint, &c_out);
        assert!(out.is_some(), "Verification should succeed with sender's PK");
        
        let (dec, verified) = out.unwrap();
        assert!(verified, "Should be quantum verified");
        assert_eq!(dec.value, Some(1337), "Value should match");
        assert_eq!(dec.r_blind, [1; 32], "Blinding should match");
    }

    #[test]
    fn verify_fails_on_tampered_timestamp() {
        // ✅ NEGATIVE TEST: Ensures replay protection via timestamp validation
        let s = QuantumKeySearchCtx::new([1; 32]).unwrap();
        let r = QuantumKeySearchCtx::new([2; 32]).unwrap();
        let c_out = [3; 32];
        let payload = HintPayloadV1 {
            r_blind: [4; 32],
            value: 42,
            memo: vec![],
        };

        let mut hint = s.build_quantum_hint(
            r.mlkem_public_key(),
            &r.x25519_public_key(),
            &c_out,
            &payload,
        ).unwrap();

        // Tamper: Push timestamp outside 2-hour window
        hint.timestamp = hint.timestamp.saturating_sub(7201);

        // Should fail verification
        assert!(
            r.verify_quantum_hint(&hint, &c_out).is_none(),
            "Verification must fail on stale timestamp"
        );
    }

    #[test]
    fn verify_fails_on_sender_pk_swap() {
        // ✅ NEGATIVE TEST: Ensures transcript binds sender PK
        use pqcrypto_traits::sign::PublicKey as PQSignPK;

        let s1 = QuantumKeySearchCtx::new([11; 32]).unwrap();
        let s2 = QuantumKeySearchCtx::new([22; 32]).unwrap();
        let r = QuantumKeySearchCtx::new([33; 32]).unwrap();
        let c_out = [44; 32];
        let payload = HintPayloadV1 {
            r_blind: [55; 32],
            value: 7,
            memo: vec![],
        };

        let mut hint = s1.build_quantum_hint(
            r.mlkem_public_key(),
            &r.x25519_public_key(),
            &c_out,
            &payload,
        ).unwrap();

        // Tamper: Swap sender PK (but signature was created by s1)
        hint.sender_falcon_pk = <FalconPublicKey as PQSignPK>::as_bytes(s2.falcon_public_key()).to_vec();

        // Should fail: transcript mismatch (PK changed)
        assert!(
            r.verify_quantum_hint(&hint, &c_out).is_none(),
            "Verification must fail on swapped sender PK"
        );
    }

    #[test]
    fn verify_fails_on_kem_ct_tamper() {
        // ✅ NEGATIVE TEST: Ensures KEM decapsulation integrity
        let s = QuantumKeySearchCtx::new([7; 32]).unwrap();
        let r = QuantumKeySearchCtx::new([9; 32]).unwrap();
        let c_out = [0xCD; 32];
        let payload = HintPayloadV1 {
            r_blind: [1; 32],
            value: 1337,
            memo: vec![],
        };

        let mut hint = s.build_quantum_hint(
            r.mlkem_public_key(),
            &r.x25519_public_key(),
            &c_out,
            &payload,
        ).unwrap();

        // Tamper: Flip bit in KEM ciphertext
        hint.kem_ct[0] ^= 0x01;

        // Should fail: Either KEM decaps fails or AEAD decryption fails
        assert!(
            r.verify_quantum_hint(&hint, &c_out).is_none(),
            "Verification must fail on tampered KEM ciphertext"
        );
    }

    #[test]
    fn verify_fails_on_x25519_pub_tamper() {
        // ✅ NEGATIVE TEST: Ensures X25519 ephemeral key integrity
        let s = QuantumKeySearchCtx::new([10; 32]).unwrap();
        let r = QuantumKeySearchCtx::new([20; 32]).unwrap();
        let c_out = [30; 32];
        let payload = HintPayloadV1 {
            r_blind: [40; 32],
            value: 9999,
            memo: vec![],
        };

        let mut hint = s.build_quantum_hint(
            r.mlkem_public_key(),
            &r.x25519_public_key(),
            &c_out,
            &payload,
        ).unwrap();

        // Tamper: Change X25519 ephemeral public key
        hint.x25519_eph_pub[0] ^= 0xFF;

        // Should fail: Transcript mismatch (X25519 pub is bound)
        assert!(
            r.verify_quantum_hint(&hint, &c_out).is_none(),
            "Verification must fail on tampered X25519 ephemeral key"
        );
    }

    #[test]
    fn verify_fails_on_encrypted_payload_tamper() {
        // ✅ NEGATIVE TEST: Ensures AEAD authentication
        let s = QuantumKeySearchCtx::new([50; 32]).unwrap();
        let r = QuantumKeySearchCtx::new([60; 32]).unwrap();
        let c_out = [70; 32];
        let payload = HintPayloadV1 {
            r_blind: [80; 32],
            value: 5555,
            memo: vec![],
        };

        let mut hint = s.build_quantum_hint(
            r.mlkem_public_key(),
            &r.x25519_public_key(),
            &c_out,
            &payload,
        ).unwrap();

        // Tamper: Flip bit in encrypted payload (AEAD ciphertext)
        hint.encrypted_payload[0] ^= 0x42;

        // Should fail: AEAD authentication tag mismatch
        assert!(
            r.verify_quantum_hint(&hint, &c_out).is_none(),
            "Verification must fail on tampered AEAD ciphertext"
        );
    }

    #[test]
    fn test_hint_fingerprint16_deterministic() {
        // ✅ TEST: Fingerprint is deterministic for same hint+c_out
        let s = QuantumKeySearchCtx::new([100; 32]).unwrap();
        let r = QuantumKeySearchCtx::new([200; 32]).unwrap();
        let c_out = [0xAB; 32];
        let payload = HintPayloadV1 {
            r_blind: [0xCD; 32],
            value: 42,
            memo: vec![],
        };

        let hint = s.build_quantum_hint(
            r.mlkem_public_key(),
            &r.x25519_public_key(),
            &c_out,
            &payload,
        ).unwrap();

        let fp1 = hint_fingerprint16(&hint, &c_out);
        let fp2 = hint_fingerprint16(&hint, &c_out);

        assert_eq!(fp1, fp2, "Fingerprint must be deterministic");
        assert_eq!(fp1.len(), 16, "Fingerprint must be exactly 16 bytes");
    }

    #[test]
    fn test_hint_fingerprint16_unique_per_hint() {
        // ✅ TEST: Different hints produce different fingerprints
        let s = QuantumKeySearchCtx::new([111; 32]).unwrap();
        let r = QuantumKeySearchCtx::new([222; 32]).unwrap();
        let c_out1 = [0x11; 32];
        let c_out2 = [0x22; 32];
        let payload = HintPayloadV1 {
            r_blind: [0x33; 32],
            value: 777,
            memo: vec![],
        };

        let hint1 = s.build_quantum_hint(
            r.mlkem_public_key(),
            &r.x25519_public_key(),
            &c_out1,
            &payload,
        ).unwrap();

        let hint2 = s.build_quantum_hint(
            r.mlkem_public_key(),
            &r.x25519_public_key(),
            &c_out2,
            &payload,
        ).unwrap();

        let fp1 = hint_fingerprint16(&hint1, &c_out1);
        let fp2 = hint_fingerprint16(&hint2, &c_out2);

        assert_ne!(fp1, fp2, "Different hints must have different fingerprints");
    }
}
