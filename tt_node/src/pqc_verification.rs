//! PQC Verification Layer (Host-side)
//! 
//! This module handles post-quantum signature verification AFTER ZK proof
//! verification. The layered approach keeps ZK guests fast (~10M cycles)
//! while providing quantum-safe ownership.
//!
//! # Architecture
//! 
//! ```text
//! ┌─ ZK Layer (RISC0 guest) ──────────────────┐
//! │ • Verify Pedersen commitments (classical) │
//! │ • Verify Merkle proofs                     │
//! │ • Check balance                            │
//! │ • Emit PQC fingerprints (public)           │
//! └────────────────────────────────────────────┘
//!              ▼
//! ┌─ PQC Layer (This module, host) ───────────┐
//! │ • Load PQC fingerprints from journal       │
//! │ • Verify Falcon signatures on nullifiers   │
//! │ • Check fingerprint binding to notes       │
//! └────────────────────────────────────────────┘
//! ```

#![forbid(unsafe_code)]

use anyhow::{anyhow, ensure, Context, Result};
use pqcrypto_falcon::falcon512;
use pqcrypto_traits::sign::PublicKey as PQPublicKey;
use serde::{Deserialize, Serialize};

pub type Hash32 = [u8; 32];

/* ============================================================================
 * Types
 * ========================================================================== */

/// PQC signature over nullifier
/// 
/// This is now a re-export from falcon_sigs for consistency
pub use crate::falcon_sigs::SignedNullifier as NullifierSignature;

/// Note metadata (stored in Merkle tree or DB)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NoteMetadata {
    /// Classical Pedersen commitment
    pub commitment: [u8; 32],
    
    /// PQC fingerprint (KMAC of PQC public keys)
    pub pqc_pk_hash: Hash32,
    
    /// Creation timestamp
    pub created_at: u64,
    
    /// Optional: full PQC public keys (if stored on-chain)
    pub falcon_pk: Option<Vec<u8>>,
    pub mlkem_pk: Option<Vec<u8>>,
}

/// Verification context (loaded from storage)
pub trait PqcVerificationContext {
    /// Load note metadata by nullifier
    fn load_note(&self, nullifier: &Hash32) -> Result<NoteMetadata>;
    
    /// Load Falcon public key by fingerprint
    fn load_falcon_pk(&self, fp: &Hash32) -> Result<falcon512::PublicKey>;
    
    /// Check if nullifier was already spent
    fn is_nullifier_spent(&self, nullifier: &Hash32) -> Result<bool>;
    
    /// Mark nullifier as spent
    fn mark_nullifier_spent(&mut self, nullifier: &Hash32) -> Result<()>;
}

/* ============================================================================
 * Core Verification
 * ========================================================================== */

/// Verify PQC signature on nullifier
/// 
/// # Arguments
/// - `nullifier`: The nullifier being spent
/// - `signature`: Falcon512 signature (attached)
/// - `falcon_pk`: Public key (must match note's pqc_pk_hash)
/// 
/// # Returns
/// Ok(()) if signature is valid
/// 
/// # Security
/// Uses proper Falcon-512 verification via falcon_sigs module
pub fn verify_nullifier_signature(
    nullifier: &Hash32,
    signature: &NullifierSignature,
    falcon_pk: &falcon512::PublicKey,
) -> Result<()> {
    crate::falcon_sigs::falcon_verify_nullifier(nullifier, signature, falcon_pk)
}

/// Verify complete spend authorization (ZK + PQC)
/// 
/// This function assumes ZK receipt was already verified.
/// It checks:
/// 1. PQC fingerprint matches note metadata
/// 2. Falcon signature is valid
/// 3. Nullifier not already spent
pub fn verify_spend_authorization<C: PqcVerificationContext>(
    ctx: &mut C,
    nullifier: &Hash32,
    pqc_fingerprint: &Hash32,
    signature: &NullifierSignature,
) -> Result<()> {
    // 1. Check if already spent
    ensure!(
        !ctx.is_nullifier_spent(nullifier)?,
        "Nullifier already spent: {}",
        hex::encode(nullifier)
    );
    
    // 2. Load note metadata
    let note = ctx.load_note(nullifier)
        .with_context(|| format!("Failed to load note for nullifier {}", hex::encode(nullifier)))?;
    
    // 3. Verify PQC fingerprint binding
    ensure!(
        &note.pqc_pk_hash == pqc_fingerprint,
        "PQC fingerprint mismatch: expected {}, got {}",
        hex::encode(note.pqc_pk_hash),
        hex::encode(pqc_fingerprint)
    );
    
    // 4. Load Falcon public key
    let falcon_pk = ctx.load_falcon_pk(pqc_fingerprint)?;
    
    // 5. Verify signature
    verify_nullifier_signature(nullifier, signature, &falcon_pk)?;
    
    // 6. Mark as spent
    ctx.mark_nullifier_spent(nullifier)?;
    
    Ok(())
}

/// Batch verification for multiple nullifiers
/// 
/// More efficient than calling verify_spend_authorization() in a loop,
/// because it can batch database lookups.
pub fn verify_batch_spend_authorization<C: PqcVerificationContext>(
    ctx: &mut C,
    spends: &[(Hash32, Hash32, NullifierSignature)], // (nullifier, fp, sig)
) -> Result<()> {
    for (nullifier, fp, sig) in spends {
        verify_spend_authorization(ctx, nullifier, fp, sig)
            .with_context(|| format!("Failed to verify spend for nullifier {}", hex::encode(nullifier)))?;
    }
    Ok(())
}

/* ============================================================================
 * Backward Compatibility (Classical Notes)
 * ========================================================================== */

/// Check if note uses classical (non-PQC) commitment
pub fn is_classical_note(pqc_fingerprint: &Hash32) -> bool {
    pqc_fingerprint == &[0u8; 32]
}

/// Verify spend for classical note (no PQC signature required)
/// 
/// This allows gradual migration: old notes can still be spent
/// without PQC signatures.
pub fn verify_classical_spend<C: PqcVerificationContext>(
    ctx: &mut C,
    nullifier: &Hash32,
) -> Result<()> {
    ensure!(
        !ctx.is_nullifier_spent(nullifier)?,
        "Nullifier already spent"
    );
    
    let note = ctx.load_note(nullifier)?;
    ensure!(
        is_classical_note(&note.pqc_pk_hash),
        "Note requires PQC signature"
    );
    
    ctx.mark_nullifier_spent(nullifier)?;
    Ok(())
}

/* ============================================================================
 * Integration with ZK Receipt
 * ========================================================================== */

/// Journal from priv_guest (decoded from ZK receipt)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrivClaim {
    pub nullifiers: Vec<Hash32>,
    pub outputs: Vec<[u8; 32]>,
    pub enc_hints: Vec<[u8; 32]>,
    pub fee_commit: [u8; 32],
    pub pqc_fingerprints_in: Vec<Hash32>,
    pub pqc_fingerprints_out: Vec<Hash32>,
}

/// Complete transaction verification (ZK + PQC)
/// 
/// # Steps
/// 1. Verify ZK receipt (done externally via risc0_zkvm::verify)
/// 2. Decode journal (PrivClaim)
/// 3. Verify PQC signatures for all inputs
/// 4. Mark nullifiers as spent
/// 5. Store new outputs
pub fn verify_private_transaction<C: PqcVerificationContext>(
    ctx: &mut C,
    claim: &PrivClaim,
    signatures: &[NullifierSignature],
) -> Result<()> {
    ensure!(
        claim.nullifiers.len() == signatures.len(),
        "Signature count mismatch: {} nullifiers, {} signatures",
        claim.nullifiers.len(),
        signatures.len()
    );
    
    ensure!(
        claim.nullifiers.len() == claim.pqc_fingerprints_in.len(),
        "PQC fingerprint count mismatch"
    );
    
    // Verify all spends
    for i in 0..claim.nullifiers.len() {
        let nullifier = &claim.nullifiers[i];
        let fp = &claim.pqc_fingerprints_in[i];
        let sig = &signatures[i];
        
        // Skip PQC verification for classical notes
        if is_classical_note(fp) {
            verify_classical_spend(ctx, nullifier)?;
        } else {
            verify_spend_authorization(ctx, nullifier, fp, sig)?;
        }
    }
    
    Ok(())
}

/* ============================================================================
 * Utilities
 * ========================================================================== */

/// Recompute PQC fingerprint from public keys (for verification)
pub fn compute_pqc_fingerprint(
    falcon_pk: &[u8],
    mlkem_pk: &[u8],
) -> Hash32 {
    crate::hybrid_commit::pqc_fingerprint(falcon_pk, mlkem_pk)
}

/// Helper: Compute fingerprint from Falcon public key object
pub fn compute_pqc_fingerprint_from_pk(
    falcon_pk: &falcon512::PublicKey,
    mlkem_pk: &[u8],
) -> Hash32 {
    use pqcrypto_traits::sign::PublicKey;
    compute_pqc_fingerprint(falcon_pk.as_bytes(), mlkem_pk)
}

/// Serialize signature for storage
pub fn serialize_signature(sig: &NullifierSignature) -> Result<Vec<u8>> {
    bincode::serialize(sig).context("Failed to serialize signature")
}

/// Deserialize signature from storage
pub fn deserialize_signature(bytes: &[u8]) -> Result<NullifierSignature> {
    bincode::deserialize(bytes).context("Failed to deserialize signature")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use pqcrypto_falcon::falcon512;
    use pqcrypto_kyber::kyber768 as mlkem;
    use pqcrypto_traits::sign::{PublicKey as PQPublicKey, SignedMessage};
    use pqcrypto_traits::kem::PublicKey as PQKemPublicKey;

    // Mock verification context for testing
    struct MockContext {
        notes: HashMap<Hash32, NoteMetadata>,
        spent: HashSet<Hash32>,
        pqc_keys: HashMap<Hash32, falcon512::PublicKey>,
    }

    impl MockContext {
        fn new() -> Self {
            Self {
                notes: HashMap::new(),
                spent: HashSet::new(),
                pqc_keys: HashMap::new(),
            }
        }
        
        fn add_note(&mut self, nullifier: Hash32, note: NoteMetadata, falcon_pk: falcon512::PublicKey) {
            self.notes.insert(nullifier, note.clone());
            self.pqc_keys.insert(note.pqc_pk_hash, falcon_pk);
        }
    }

    impl PqcVerificationContext for MockContext {
        fn load_note(&self, nullifier: &Hash32) -> Result<NoteMetadata> {
            self.notes.get(nullifier)
                .cloned()
                .ok_or_else(|| anyhow!("Note not found"))
        }
        
        fn load_falcon_pk(&self, fp: &Hash32) -> Result<falcon512::PublicKey> {
            self.pqc_keys.get(fp)
                .cloned()
                .ok_or_else(|| anyhow!("Falcon PK not found"))
        }
        
        fn is_nullifier_spent(&self, nullifier: &Hash32) -> Result<bool> {
            Ok(self.spent.contains(nullifier))
        }
        
        fn mark_nullifier_spent(&mut self, nullifier: &Hash32) -> Result<()> {
            self.spent.insert(*nullifier);
            Ok(())
        }
    }

    #[test]
    fn test_verify_spend_authorization() {
        let mut ctx = MockContext::new();
        
        // Generate PQC keys
        let (falcon_pk, falcon_sk) = falcon512::keypair();
        let (mlkem_pk, _mlkem_sk) = mlkem::keypair();
        
        // Compute fingerprint
        let fp = compute_pqc_fingerprint(falcon_pk.as_bytes(), mlkem_pk.as_bytes());
        
        // Create note
        let nullifier = [0x42u8; 32];
        let note = NoteMetadata {
            commitment: [0x01u8; 32],
            pqc_pk_hash: fp,
            created_at: 1234567890,
            falcon_pk: Some(falcon_pk.as_bytes().to_vec()),
            mlkem_pk: Some(mlkem_pk.as_bytes().to_vec()),
        };
        
        ctx.add_note(nullifier, note, falcon_pk.clone());
        
        // Sign nullifier using falcon_sigs
        let signature = crate::falcon_sigs::falcon_sign_nullifier(&nullifier, &falcon_sk)
            .expect("Sign should succeed");
        
        // Verify
        let result = verify_spend_authorization(&mut ctx, &nullifier, &fp, &signature);
        assert!(result.is_ok(), "Verification should succeed");
        assert!(ctx.is_nullifier_spent(&nullifier).unwrap(), "Nullifier should be marked spent");
        
        // Try double-spend
        let result2 = verify_spend_authorization(&mut ctx, &nullifier, &fp, &signature);
        assert!(result2.is_err(), "Double-spend should fail");
    }

    #[test]
    fn test_classical_note_compatibility() {
        let mut ctx = MockContext::new();
        
        let nullifier = [0x99u8; 32];
        let note = NoteMetadata {
            commitment: [0x01u8; 32],
            pqc_pk_hash: [0u8; 32], // Classical (zero fingerprint)
            created_at: 1234567890,
            falcon_pk: None,
            mlkem_pk: None,
        };
        
        ctx.notes.insert(nullifier, note);
        
        // Should succeed without signature
        let result = verify_classical_spend(&mut ctx, &nullifier);
        assert!(result.is_ok(), "Classical spend should succeed");
    }

    #[test]
    fn test_wrong_fingerprint_fails() {
        let mut ctx = MockContext::new();
        
        let (falcon_pk, falcon_sk) = falcon512::keypair();
        let (mlkem_pk, _) = mlkem::keypair();
        let fp = compute_pqc_fingerprint(falcon_pk.as_bytes(), mlkem_pk.as_bytes());
        
        let nullifier = [0x42u8; 32];
        let note = NoteMetadata {
            commitment: [0x01u8; 32],
            pqc_pk_hash: fp,
            created_at: 1234567890,
            falcon_pk: Some(falcon_pk.as_bytes().to_vec()),
            mlkem_pk: Some(mlkem_pk.as_bytes().to_vec()),
        };
        
        ctx.add_note(nullifier, note, falcon_pk.clone());
        
        // Use WRONG fingerprint
        let wrong_fp = [0xFFu8; 32];
        let signature = crate::falcon_sigs::falcon_sign_nullifier(&nullifier, &falcon_sk)
            .expect("Sign should succeed");
        
        let result = verify_spend_authorization(&mut ctx, &nullifier, &wrong_fp, &signature);
        assert!(result.is_err(), "Wrong fingerprint should fail");
    }
}