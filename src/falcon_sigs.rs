//! Falcon-512 Post-Quantum Signatures for Blockchain
//! 
//! Adapted from quantum_falcon_wallet for blockchain block signing.
//! 
//! # Security Model
//! - **Attached signatures**: Sign message, verify by opening
//! - **Block signing**: Sign 32-byte block hashes
//! - **Public key fingerprints**: For Node ID derivation
//!
//! # Example
//! ```no_run
//! use tt_priv_cli::falcon_sigs::*;
//! 
//! let (pk, sk) = falcon_keypair();
//! let block_hash = [0x42u8; 32];
//! let sig = falcon_sign_block(&block_hash, &sk);
//! falcon_verify_block(&block_hash, &sig, &pk).unwrap();
//! ```

#![forbid(unsafe_code)]

use anyhow::{anyhow, ensure, Result};
use pqcrypto_falcon::falcon512;
use pqcrypto_traits::sign::{PublicKey as PQPublicKey, SecretKey as PQSecretKey, SignedMessage};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

pub type Hash32 = [u8; 32];

/* ============================================================================
 * Core Types
 * ========================================================================== */

/// Falcon-512 public key (897 bytes)
pub type FalconPublicKey = falcon512::PublicKey;

/// Falcon-512 secret key (1281 bytes, zeroized on drop)
pub type FalconSecretKey = falcon512::SecretKey;

/// Block signature (attached signature format)
/// 
/// Contains:
/// - Original block hash (32 bytes)
/// - Falcon signature (~666 bytes)
/// Total: ~698 bytes
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockSignature {
    /// Attached signature (message + sig)
    pub signed_message_bytes: Vec<u8>,
}

impl Zeroize for BlockSignature {
    fn zeroize(&mut self) {
        self.signed_message_bytes.zeroize();
    }
}

impl Drop for BlockSignature {
    fn drop(&mut self) {
        self.zeroize();
    }
}

/* ============================================================================
 * Key Generation
 * ========================================================================== */

/// Generate new Falcon-512 keypair (random)
///
/// **WARNING:** For consensus/validators, use `falcon_keypair_from_seed()` instead!
#[inline]
pub fn falcon_keypair() -> (FalconPublicKey, FalconSecretKey) {
    falcon512::keypair()
}

/// Generate deterministic Falcon-512 keypair from seed
///
/// **CRITICAL FOR CONSENSUS:** Validators MUST use deterministic keys!
///
/// # Parameters
/// - `seed`: 32-byte secret seed (e.g., node master key)
/// - `domain`: Context string for domain separation (e.g., b"consensus/validator")
///
/// # Example
/// ```no_run
/// use tt_priv_cli::falcon_sigs::*;
/// 
/// let node_seed: [u8; 32] = /* derive from master secret */;
/// let (pk, sk) = falcon_keypair_from_seed(&node_seed, b"consensus/validator");
/// // Same seed + domain always produces same keypair
/// ```
///
/// # Security
/// - Seed must have ≥256 bits entropy
/// - Domain prevents cross-context attacks
/// - Same (seed, domain) → same keypair (deterministic)
///
/// # Implementation
/// - **With `seeded_falcon` feature**: TRUE deterministic keygen via PQClean FFI + KMAC-DRBG
/// - **Without feature**: Falls back to random keygen (for testing only!)
pub fn falcon_keypair_from_seed(
    seed: &[u8; 32],
    domain: &[u8],
) -> (FalconPublicKey, FalconSecretKey) {
    #[cfg(feature = "seeded_falcon")]
    {
        // ✅ TRUE DETERMINISTIC via falcon_seeded FFI
        use crate::crypto::seeded::falcon_keypair_deterministic;
        
        match falcon_keypair_deterministic(*seed, domain) {
            Ok((pk_bytes, sk_bytes)) => {
                // Convert raw bytes to pqcrypto types
                let pk = FalconPublicKey::from_bytes(&pk_bytes)
                    .expect("Valid Falcon PK from deterministic keygen");
                let sk = FalconSecretKey::from_bytes(&sk_bytes)
                    .expect("Valid Falcon SK from deterministic keygen");
                (pk, sk)
            }
            Err(e) => {
                panic!("Deterministic Falcon keygen failed: {}", e);
            }
        }
    }
    
    #[cfg(not(feature = "seeded_falcon"))]
    {
        // ⚠️ FALLBACK: Random keygen (NOT deterministic!)
        // This should NEVER be used in production consensus!
        eprintln!("⚠️  WARNING: Using RANDOM Falcon keygen (seeded_falcon feature disabled)");
        eprintln!("⚠️  For production consensus, enable seeded_falcon feature!");
        eprintln!("⚠️  Seed: {:?}, Domain: {:?}", seed, domain);
        
        falcon512::keypair()
    }
}

/// Import public key from bytes
pub fn falcon_pk_from_bytes(bytes: &[u8]) -> Result<FalconPublicKey> {
    FalconPublicKey::from_bytes(bytes)
        .map_err(|_| anyhow!("Invalid Falcon public key bytes"))
}

/// Import secret key from bytes
pub fn falcon_sk_from_bytes(bytes: &[u8]) -> Result<FalconSecretKey> {
    FalconSecretKey::from_bytes(bytes)
        .map_err(|_| anyhow!("Invalid Falcon secret key bytes"))
}

/// Export public key to bytes (897 bytes)
#[inline]
pub fn falcon_pk_to_bytes(pk: &FalconPublicKey) -> &[u8] {
    pk.as_bytes()
}

/// Export secret key to bytes (1281 bytes) - SENSITIVE!
#[inline]
pub fn falcon_sk_to_bytes(sk: &FalconSecretKey) -> Zeroizing<Vec<u8>> {
    Zeroizing::new(sk.as_bytes().to_vec())
}

/* ============================================================================
 * Block Signing (Blockchain Specific)
 * ========================================================================== */

/// Sign a block hash with Falcon-512
/// 
/// # Performance
/// ~10ms on modern CPU
pub fn falcon_sign_block(
    block_hash: &Hash32,
    secret_key: &FalconSecretKey,
) -> BlockSignature {
    let signed_msg = falcon512::sign(block_hash, secret_key);
    
    BlockSignature {
        signed_message_bytes: signed_msg.as_bytes().to_vec(),
    }
}

/// Verify Falcon-512 signature on block
/// 
/// # Performance
/// ~200 microseconds on modern CPU
pub fn falcon_verify_block(
    block_hash: &Hash32,
    signature: &BlockSignature,
    public_key: &FalconPublicKey,
) -> Result<()> {
    // Parse signed message
    let signed_msg = falcon512::SignedMessage::from_bytes(&signature.signed_message_bytes)
        .map_err(|_| anyhow!("Invalid Falcon SignedMessage format"))?;
    
    // Open (verify + extract message)
    let recovered_msg = falcon512::open(&signed_msg, public_key)
        .map_err(|_| anyhow!("Falcon signature verification failed"))?;
    
    // Check message matches expected block hash
    ensure!(
        recovered_msg.len() == 32,
        "Recovered message length mismatch: expected 32, got {}",
        recovered_msg.len()
    );
    
    ensure!(
        recovered_msg.as_slice() == block_hash,
        "Block hash mismatch: signature is for different block"
    );
    
    Ok(())
}

/* ============================================================================
 * Node ID Derivation
 * ========================================================================== */

/// Derive Node ID from Falcon public key (for PoT consensus)
pub fn derive_node_id(falcon_pk: &FalconPublicKey) -> Hash32 {
    use tiny_keccak::{Hasher, Shake};
    let mut sh = Shake::v256();
    sh.update(b"TT_NODE_ID");
    sh.update(falcon_pk.as_bytes());
    let mut out = [0u8; 32];
    sh.finalize(&mut out);
    out
}

/* ============================================================================
 * Utilities
 * ========================================================================== */

/// Check if public key is valid (can be parsed)
pub fn is_valid_falcon_pk(bytes: &[u8]) -> bool {
    FalconPublicKey::from_bytes(bytes).is_ok()
}

/// Get signature size estimate
#[inline]
pub const fn falcon_signature_size_estimate() -> usize {
    698 // message + signature
}

/// Get public key size
#[inline]
pub const fn falcon_pk_size() -> usize {
    897
}

/// Get secret key size
#[inline]
pub const fn falcon_sk_size() -> usize {
    1281
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let (pk, sk) = falcon_keypair();
        assert_eq!(pk.as_bytes().len(), falcon_pk_size());
        assert_eq!(sk.as_bytes().len(), falcon_sk_size());
    }

    #[test]
    fn test_sign_verify_block() {
        let (pk, sk) = falcon_keypair();
        let block_hash = [0x42u8; 32];
        
        let sig = falcon_sign_block(&block_hash, &sk);
        
        let result = falcon_verify_block(&block_hash, &sig, &pk);
        assert!(result.is_ok(), "Verification should succeed");
    }

    #[test]
    fn test_wrong_block_hash_fails() {
        let (pk, sk) = falcon_keypair();
        let block_hash = [0x42u8; 32];
        let wrong_hash = [0x99u8; 32];
        
        let sig = falcon_sign_block(&block_hash, &sk);
        
        let result = falcon_verify_block(&wrong_hash, &sig, &pk);
        assert!(result.is_err(), "Wrong hash should fail");
    }

    #[test]
    fn test_node_id_derivation() {
        let (pk, _) = falcon_keypair();
        let node_id = derive_node_id(&pk);
        
        assert_eq!(node_id.len(), 32);
        
        // Deterministic
        let node_id2 = derive_node_id(&pk);
        assert_eq!(node_id, node_id2);
    }

    #[test]
    fn test_key_import_export() {
        let (pk, sk) = falcon_keypair();
        
        let pk_bytes = falcon_pk_to_bytes(&pk);
        let sk_bytes = falcon_sk_to_bytes(&sk);
        
        let pk2 = falcon_pk_from_bytes(pk_bytes).unwrap();
        let sk2 = falcon_sk_from_bytes(&sk_bytes).unwrap();
        
        let block_hash = [0x42u8; 32];
        let sig = falcon_sign_block(&block_hash, &sk2);
        
        falcon_verify_block(&block_hash, &sig, &pk2).unwrap();
    }
}
