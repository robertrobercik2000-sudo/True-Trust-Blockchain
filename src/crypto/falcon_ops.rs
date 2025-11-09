//! Unified Falcon operations facade
//!
//! This module provides a trait-based abstraction over Falcon-512 operations,
//! with compile-time selection between:
//! - Default: `pqcrypto-falcon` (OS RNG, non-deterministic)
//! - Optional: `falcon_seeded` (KMAC-DRBG, deterministic)
//!
//! This consolidates all `#[cfg(feature = "seeded_falcon")]` in one place.

#![forbid(unsafe_code)]

use pqcrypto_traits::sign::{PublicKey as PQPublicKey, SecretKey as PQSecretKey};

/* ===== TYPE ALIASES ===== */

pub type FalconPublicKey = pqcrypto_falcon::falcon512::PublicKey;
pub type FalconSecretKey = pqcrypto_falcon::falcon512::SecretKey;

/* ===== UNIFIED TRAIT ===== */

/// Unified interface for Falcon-512 operations.
///
/// This trait abstracts over different Falcon implementations to enable
/// compile-time selection via feature flags without polluting call sites.
pub trait FalconOps {
    /// Generate Falcon-512 keypair
    fn keypair(&self) -> (FalconPublicKey, FalconSecretKey);

    /// Sign message with Falcon-512 (attached signature)
    fn sign(&self, msg: &[u8], sk: &FalconSecretKey) -> Vec<u8>;

    /// Verify Falcon-512 signature and recover message
    fn verify(&self, signed_msg: &[u8], pk: &FalconPublicKey) -> Option<Vec<u8>>;
}

/* ===== DEFAULT IMPLEMENTATION (pqcrypto-falcon) ===== */

#[cfg(not(feature = "seeded_falcon"))]
pub struct DefaultFalconOps;

#[cfg(not(feature = "seeded_falcon"))]
impl FalconOps for DefaultFalconOps {
    fn keypair(&self) -> (FalconPublicKey, FalconSecretKey) {
        pqcrypto_falcon::falcon512::keypair()
    }

    fn sign(&self, msg: &[u8], sk: &FalconSecretKey) -> Vec<u8> {
        use pqcrypto_traits::sign::SignedMessage;
        let signed = pqcrypto_falcon::falcon512::sign(msg, sk);
        signed.as_bytes().to_vec()
    }

    fn verify(&self, signed_msg: &[u8], pk: &FalconPublicKey) -> Option<Vec<u8>> {
        use pqcrypto_traits::sign::SignedMessage;
        // Reconstruct SignedMessage from bytes
        let sm = <pqcrypto_falcon::falcon512::SignedMessage as SignedMessage>::from_bytes(signed_msg).ok()?;
        pqcrypto_falcon::falcon512::open(&sm, pk).ok()
    }
}

/* ===== SEEDED IMPLEMENTATION (falcon_seeded + KMAC-DRBG) ===== */

#[cfg(feature = "seeded_falcon")]
pub struct SeededFalconOps {
    /// Master seed for deterministic operations
    seed: [u8; 32],
}

#[cfg(feature = "seeded_falcon")]
impl SeededFalconOps {
    /// Create seeded Falcon ops with master seed
    pub fn new(seed: [u8; 32]) -> Self {
        Self { seed }
    }

    /// Derive keypair seed from master seed + personalization
    fn derive_keyseed(&self, personalization: &[u8]) -> [u8; 32] {
        use crate::crypto::kmac::kmac256_derive_key;
        kmac256_derive_key(&self.seed, b"FALCON/KEYGEN", personalization)
    }

    /// Derive signing coins seed from secret key + message
    fn derive_coins(&self, sk: &FalconSecretKey, msg: &[u8]) -> [u8; 32] {
        use crate::crypto::kmac::kmac256_derive_key;
        // Extract PRF key from secret key (first 32 bytes of serialized SK)
        let sk_bytes = sk.as_bytes();
        let sk_prf = &sk_bytes[..32.min(sk_bytes.len())];
        kmac256_derive_key(sk_prf, b"FALCON/SIGN/COINS", msg)
    }
}

#[cfg(feature = "seeded_falcon")]
impl FalconOps for SeededFalconOps {
    fn keypair(&self) -> (FalconPublicKey, FalconSecretKey) {
        use crate::crypto::seeded::falcon_keypair_deterministic;
        let keyseed = self.derive_keyseed(b"default");
        let (pk_bytes, sk_bytes) = falcon_keypair_deterministic(keyseed, b"v1")
            .expect("Deterministic keygen failed");

        // Convert to pqcrypto types (assumes compatible serialization)
        let pk = FalconPublicKey::from_bytes(&pk_bytes)
            .expect("Invalid Falcon PK");
        let sk = FalconSecretKey::from_bytes(&sk_bytes)
            .expect("Invalid Falcon SK");
        (pk, sk)
    }

    fn sign(&self, msg: &[u8], sk: &FalconSecretKey) -> Vec<u8> {
        use crate::crypto::seeded::falcon_sign_deterministic;
        let coins = self.derive_coins(sk, msg);
        let sk_bytes: [u8; 1281] = sk.as_bytes().try_into()
            .expect("Falcon SK must be 1281 bytes");
        falcon_sign_deterministic(&sk_bytes, msg, coins, b"v1")
            .expect("Deterministic signing failed")
    }

    fn verify(&self, signed_msg: &[u8], pk: &FalconPublicKey) -> Option<Vec<u8>> {
        use pqcrypto_traits::sign::SignedMessage;
        // Reconstruct SignedMessage from bytes
        let sm = <pqcrypto_falcon::falcon512::SignedMessage as SignedMessage>::from_bytes(signed_msg).ok()?;
        pqcrypto_falcon::falcon512::open(&sm, pk).ok()
    }
}

/* ===== GLOBAL FACADE ===== */

/// Get the default Falcon operations implementation.
///
/// This returns either `DefaultFalconOps` or `SeededFalconOps` based on
/// the `seeded_falcon` feature flag at compile time.
#[cfg(not(feature = "seeded_falcon"))]
pub fn default_falcon_ops() -> impl FalconOps {
    DefaultFalconOps
}

#[cfg(feature = "seeded_falcon")]
pub fn default_falcon_ops() -> impl FalconOps {
    // Use zero seed for default (caller should override with proper seed)
    SeededFalconOps::new([0u8; 32])
}

/* ===== TESTS ===== */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_ops_roundtrip() {
        let ops = default_falcon_ops();
        let (pk, sk) = ops.keypair();
        
        let msg = b"Hello, Falcon!";
        let signed = ops.sign(msg, &sk);
        let recovered = ops.verify(&signed, &pk);
        
        assert_eq!(recovered, Some(msg.to_vec()));
    }

    #[test]
    fn test_wrong_pk_fails() {
        let ops = default_falcon_ops();
        let (pk1, sk1) = ops.keypair();
        let (pk2, _sk2) = ops.keypair();
        
        let signed = ops.sign(b"test", &sk1);
        let recovered = ops.verify(&signed, &pk2);
        
        assert!(recovered.is_none());
    }

    #[cfg(feature = "seeded_falcon")]
    #[test]
    #[ignore] // Requires PQClean setup
    fn test_seeded_ops_determinism() {
        let seed = [0x42u8; 32];
        let ops1 = SeededFalconOps::new(seed);
        let ops2 = SeededFalconOps::new(seed);
        
        let (pk1, sk1) = ops1.keypair();
        let (pk2, sk2) = ops2.keypair();
        
        assert_eq!(pk1.as_bytes(), pk2.as_bytes());
        assert_eq!(sk1.as_bytes(), sk2.as_bytes());
    }
}
