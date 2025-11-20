//! Kyber-768 Post-Quantum Key Encapsulation Mechanism
//! 
//! For secure peer-to-peer channel establishment in the blockchain network.
//!
//! # Security Model
//! - **IND-CCA2 secure**: Chosen ciphertext attack resistant
//! - **Key exchange**: Establish shared secrets between nodes
//! - **Forward secrecy**: Each session uses fresh shared secret
//!
//! # Example
//! ```no_run
//! use tt_priv_cli::kyber_kem::*;
//! 
//! // Recipient generates keypair
//! let (recipient_pk, recipient_sk) = kyber_keypair();
//! 
//! // Sender encapsulates to get shared secret + ciphertext
//! let (sender_ss, ciphertext) = kyber_encapsulate(&recipient_pk);
//! 
//! // Recipient decapsulates to recover shared secret
//! let recipient_ss = kyber_decapsulate(&ciphertext, &recipient_sk).unwrap();
//! 
//! assert_eq!(sender_ss.as_bytes(), recipient_ss.as_bytes());
//! ```

#![forbid(unsafe_code)]

use anyhow::{anyhow, Result};
use pqcrypto_kyber::kyber768;
use pqcrypto_traits::kem::{
    PublicKey as KemPublicKey,
    SecretKey as KemSecretKey,
    SharedSecret as KemSharedSecret,
    Ciphertext as KemCiphertext,
};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

pub type Hash32 = [u8; 32];

/* ============================================================================
 * Core Types
 * ========================================================================== */

/// Kyber-768 public key (1184 bytes)
pub type KyberPublicKey = kyber768::PublicKey;

/// Kyber-768 secret key (2400 bytes, zeroized on drop)
pub type KyberSecretKey = kyber768::SecretKey;

/// Kyber-768 shared secret (32 bytes, zeroized on drop)
pub type KyberSharedSecret = kyber768::SharedSecret;

/// Kyber-768 ciphertext (1088 bytes)
pub type KyberCiphertext = kyber768::Ciphertext;

/* ============================================================================
 * Key Generation
 * ========================================================================== */

/// Generate new Kyber-768 keypair
/// 
/// # Returns
/// (public_key, secret_key)
#[inline]
pub fn kyber_keypair() -> (KyberPublicKey, KyberSecretKey) {
    kyber768::keypair()
}

/// Import public key from bytes
pub fn kyber_pk_from_bytes(bytes: &[u8]) -> Result<KyberPublicKey> {
    KyberPublicKey::from_bytes(bytes)
        .map_err(|_| anyhow!("Invalid Kyber public key bytes"))
}

/// Import secret key from bytes
pub fn kyber_sk_from_bytes(bytes: &[u8]) -> Result<KyberSecretKey> {
    KyberSecretKey::from_bytes(bytes)
        .map_err(|_| anyhow!("Invalid Kyber secret key bytes"))
}

/// Export public key to bytes (1184 bytes)
#[inline]
pub fn kyber_pk_to_bytes(pk: &KyberPublicKey) -> &[u8] {
    pk.as_bytes()
}

/// Export secret key to bytes (2400 bytes) - SENSITIVE!
#[inline]
pub fn kyber_sk_to_bytes(sk: &KyberSecretKey) -> Zeroizing<Vec<u8>> {
    Zeroizing::new(sk.as_bytes().to_vec())
}

/* ============================================================================
 * Encapsulation / Decapsulation
 * ========================================================================== */

/// Encapsulate to generate shared secret
/// 
/// # Returns
/// (shared_secret, ciphertext)
/// 
/// # Performance
/// ~200 microseconds on modern CPU
#[inline]
pub fn kyber_encapsulate(public_key: &KyberPublicKey) -> (KyberSharedSecret, KyberCiphertext) {
    kyber768::encapsulate(public_key)
}

/// Decapsulate ciphertext to recover shared secret
/// 
/// # Performance
/// ~300 microseconds on modern CPU
pub fn kyber_decapsulate(
    ciphertext: &KyberCiphertext,
    secret_key: &KyberSecretKey,
) -> Result<KyberSharedSecret> {
    Ok(kyber768::decapsulate(ciphertext, secret_key))
}

/* ============================================================================
 * Ciphertext Handling
 * ========================================================================== */

/// Import ciphertext from bytes
pub fn kyber_ct_from_bytes(bytes: &[u8]) -> Result<KyberCiphertext> {
    KyberCiphertext::from_bytes(bytes)
        .map_err(|_| anyhow!("Invalid Kyber ciphertext bytes"))
}

/// Export ciphertext to bytes (1088 bytes)
#[inline]
pub fn kyber_ct_to_bytes(ct: &KyberCiphertext) -> &[u8] {
    ct.as_bytes()
}

/* ============================================================================
 * Shared Secret Handling
 * ========================================================================== */

/// Export shared secret to bytes (32 bytes) - SENSITIVE!
#[inline]
pub fn kyber_ss_to_bytes(ss: &KyberSharedSecret) -> Zeroizing<Vec<u8>> {
    Zeroizing::new(ss.as_bytes().to_vec())
}

/// Derive symmetric key from shared secret using KMAC256
pub fn derive_aes_key_from_shared_secret(ss: &KyberSharedSecret, context: &[u8]) -> [u8; 32] {
    use crate::crypto_kmac_consensus::kmac256_hash;
    kmac256_hash(context, &[ss.as_bytes()])
}

/// Derive 32-byte AES key from shared secret bytes (for use with Zeroizing wrapper)
pub fn derive_aes_key_from_shared_secret_bytes(ss_bytes: &[u8], context: &[u8]) -> [u8; 32] {
    use crate::crypto_kmac_consensus::kmac256_hash;
    kmac256_hash(context, &[ss_bytes])
}

/* ============================================================================
 * High-Level API
 * ========================================================================== */

/// Serializable key exchange result
#[derive(Clone, Serialize, Deserialize)]
pub struct KeyExchangeInitiator {
    pub shared_secret_bytes: Vec<u8>,  // Zeroized
    pub ciphertext_bytes: Vec<u8>,
}

impl Zeroize for KeyExchangeInitiator {
    fn zeroize(&mut self) {
        self.shared_secret_bytes.zeroize();
    }
}

impl Drop for KeyExchangeInitiator {
    fn drop(&mut self) {
        self.zeroize();
    }
}

/// Initiate key exchange (sender side)
pub fn initiate_key_exchange(recipient_pk: &KyberPublicKey) -> KeyExchangeInitiator {
    let (ss, ct) = kyber_encapsulate(recipient_pk);
    KeyExchangeInitiator {
        shared_secret_bytes: ss.as_bytes().to_vec(),
        ciphertext_bytes: ct.as_bytes().to_vec(),
    }
}

/// Complete key exchange (recipient side)
pub fn complete_key_exchange(
    ciphertext_bytes: &[u8],
    recipient_sk: &KyberSecretKey,
) -> Result<Zeroizing<Vec<u8>>> {
    let ct = kyber_ct_from_bytes(ciphertext_bytes)?;
    let ss = kyber_decapsulate(&ct, recipient_sk)?;
    Ok(Zeroizing::new(ss.as_bytes().to_vec()))
}

/* ============================================================================
 * Utilities
 * ========================================================================== */

/// Get public key size
#[inline]
pub const fn kyber_pk_size() -> usize {
    1184
}

/// Get secret key size
#[inline]
pub const fn kyber_sk_size() -> usize {
    2400
}

/// Get ciphertext size
#[inline]
pub const fn kyber_ct_size() -> usize {
    1088
}

/// Get shared secret size
#[inline]
pub const fn kyber_ss_size() -> usize {
    32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let (pk, sk) = kyber_keypair();
        assert_eq!(pk.as_bytes().len(), kyber_pk_size());
        assert_eq!(sk.as_bytes().len(), kyber_sk_size());
    }

    #[test]
    fn test_encapsulate_decapsulate() {
        let (pk, sk) = kyber_keypair();
        
        let (ss1, ct) = kyber_encapsulate(&pk);
        let ss2 = kyber_decapsulate(&ct, &sk).unwrap();
        
        assert_eq!(ss1.as_bytes(), ss2.as_bytes(), "Shared secrets must match");
    }

    #[test]
    fn test_key_exchange_api() {
        let (recipient_pk, recipient_sk) = kyber_keypair();
        
        // Initiator
        let kex_init = initiate_key_exchange(&recipient_pk);
        
        // Recipient
        let ss_recipient = complete_key_exchange(
            &kex_init.ciphertext_bytes,
            &recipient_sk,
        ).unwrap();
        
        assert_eq!(
            kex_init.shared_secret_bytes.as_slice(),
            ss_recipient.as_slice(),
            "Shared secrets must match"
        );
    }

    #[test]
    fn test_derive_symmetric_key() {
        let (pk, _) = kyber_keypair();
        let (ss, _) = kyber_encapsulate(&pk);
        
        let key1 = derive_aes_key_from_shared_secret(&ss, b"CHANNEL_ENC");
        let key2 = derive_aes_key_from_shared_secret(&ss, b"CHANNEL_MAC");
        
        assert_ne!(key1, key2, "Different contexts should derive different keys");
    }

    #[test]
    fn test_ciphertext_import_export() {
        let (pk, sk) = kyber_keypair();
        let (ss1, ct) = kyber_encapsulate(&pk);
        
        // Export
        let ct_bytes = kyber_ct_to_bytes(&ct);
        assert_eq!(ct_bytes.len(), kyber_ct_size());
        
        // Import
        let ct2 = kyber_ct_from_bytes(ct_bytes).unwrap();
        let ss2 = kyber_decapsulate(&ct2, &sk).unwrap();
        
        assert_eq!(ss1.as_bytes(), ss2.as_bytes());
    }
}