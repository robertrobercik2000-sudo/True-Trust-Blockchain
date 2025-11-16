#![forbid(unsafe_code)]

//! Post-Quantum Transaction Module (STARK-based)
//!
//! Replaces Bulletproofs (ECC-based, NOT quantum-safe)
//! with STARK range proofs (hash-based, 100% quantum-safe).
//!
//! ## Security:
//! - ✅ STARK range proofs (256-bit quantum security)
//! - ✅ SHA3-256 commitments (collision-resistant)
//! - ✅ Falcon512 signatures (NIST PQC)
//!
//! ## Performance:
//! - Prove: ~500ms/output (vs ~10ms for BP)
//! - Verify: ~50ms/output (vs ~5ms for BP)
//! - Size: ~50KB/proof (vs ~700B for BP)
//!
//! Trade-off is acceptable for L1 blockchain security.

use serde::{Serialize, Deserialize};
use sha3::{Sha3_256, Digest};
use rand::RngCore;

use crate::stark_full::{
    STARKProver, STARKVerifier, STARKProof,
    encode_range_public_inputs, decode_commitment_from_public_inputs,
    RANGE_PROOF_PUBLIC_INPUTS_LEN,
};
use crate::core::Hash32;

/// Kyber768 ciphertext size (1088 bytes)
const KYBER768_CT_BYTES: usize = 1088;

// =================== Transaction Output (STARK) ===================

/// Transaction output with STARK range proof
///
/// Replaces Pedersen commitment (ECC) with hash commitment (PQ-safe).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TxOutputStark {
    /// Hash commitment: SHA3-256(value || blinding || recipient)
    ///
    /// This proves output integrity without revealing value.
    pub value_commitment: Hash32,
    
    /// STARK range proof: proves 0 ≤ value < 2^64
    ///
    /// Size: ~50 KB (vs ~700B for Bulletproofs)
    /// Verify time: ~50ms (vs ~5ms for BP)
    pub stark_proof: Vec<u8>,
    
    /// Recipient address (hash of Falcon512 public key)
    pub recipient: Hash32,
    
    /// Encrypted value (for recipient only)
    ///
    /// Encrypted with recipient's Kyber768 public key.
    /// Only recipient can decrypt to spend.
    pub encrypted_value: Vec<u8>, // Kyber768 ciphertext (~1088 bytes)
}

impl TxOutputStark {
    /// Create new output with STARK proof
    ///
    /// # Arguments
    /// * `value` - Amount (0 ≤ value < 2^64)
    /// * `blinding` - Random 32-byte blinding factor
    /// * `recipient` - Recipient's address (Falcon PK hash)
    /// * `recipient_kyber_pk` - Recipient's Kyber768 public key (for encryption)
    ///
    /// # Performance
    /// - STARK proof generation: ~500ms
    /// - Kyber encryption: ~200μs
    pub fn new(
        value: u64,
        blinding: &[u8; 32],
        recipient: Hash32,
        recipient_kyber_pk: &crate::kyber_kem::KyberPublicKey,
    ) -> Self {
        // 1. Commitment: SHA3-256(value || blinding || recipient)
        let mut h = Sha3_256::new();
        h.update(b"TX_OUTPUT_STARK.v1");
        h.update(&value.to_le_bytes());
        h.update(blinding);
        h.update(&recipient);
        let commitment: Hash32 = h.finalize().into();
        
        // 2. STARK range proof BOUND to commitment (prevents proof reuse!)
        let proof = STARKProver::prove_range_with_commitment(value, &commitment);
        let stark_proof = bincode::serialize(&proof)
            .expect("STARK proof serialization failed");
        
        // 3. Encrypt value for recipient (Kyber768)
        let (ss, ct) = crate::kyber_kem::kyber_encapsulate(recipient_kyber_pk);
        
        // Derive AES key from shared secret
        let ss_bytes = crate::kyber_kem::kyber_ss_to_bytes(&ss);
        let aes_key = crate::kyber_kem::derive_aes_key_from_shared_secret_bytes(&ss_bytes, b"TX_VALUE_ENC");
        
        // Encrypt value + blinding
        use chacha20poly1305::{XChaCha20Poly1305, Key, XNonce, aead::{Aead, KeyInit}};
        let cipher = XChaCha20Poly1305::new(Key::from_slice(&aes_key));
        let mut nonce_bytes = [0u8; 24];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = XNonce::from(nonce_bytes);
        
        let mut plaintext = Vec::with_capacity(8 + 32);
        plaintext.extend_from_slice(&value.to_le_bytes());
        plaintext.extend_from_slice(blinding);
        
        let ciphertext = cipher.encrypt(&nonce, plaintext.as_ref())
            .expect("Encryption failed");
        
        // Combine: nonce + ciphertext + kyber_ct
        let ct_bytes = crate::kyber_kem::kyber_ct_to_bytes(&ct);
        let mut encrypted_value = Vec::with_capacity(24 + ciphertext.len() + ct_bytes.len());
        encrypted_value.extend_from_slice(&nonce_bytes);
        encrypted_value.extend_from_slice(&ciphertext);
        encrypted_value.extend_from_slice(ct_bytes);
        
        Self {
            value_commitment: commitment,
            stark_proof,
            recipient,
            encrypted_value,
        }
    }
    
    /// Verify STARK range proof + commitment binding
    ///
    /// Performs TWO critical checks:
    /// 1. **Commitment binding**: Proof's embedded commitment must match `self.value_commitment`
    /// 2. **Range constraint**: Proof must validate `0 ≤ value < 2^64`
    ///
    /// **Security**: Prevents proof reuse attacks. A proof generated for output A
    /// cannot be used for output B (different commitments).
    ///
    /// # Performance
    /// ~50ms (vs ~5ms for Bulletproofs)
    pub fn verify(&self) -> bool {
        // 1. Deserialize proof
        let proof: STARKProof = match bincode::deserialize(&self.stark_proof) {
            Ok(p) => p,
            Err(_) => return false,
        };

        // 2. Check public inputs layout
        if proof.public_inputs.len() != RANGE_PROOF_PUBLIC_INPUTS_LEN {
            return false; // Expected: [value, c0, c1, c2, c3]
        }

        // 3. **CRITICAL: Commitment binding check**
        if let Some(comm_from_proof) = decode_commitment_from_public_inputs(&proof.public_inputs) {
            if comm_from_proof != self.value_commitment {
                // ❌ Proof was generated for a different output!
                return false;
            }
        } else {
            return false;
        }

        // 4. Verify STARK proof itself (range constraint + FRI)
        STARKVerifier::verify(&proof)
    }
    
    /// Decrypt value (recipient only) - raw (value, blinding)
    ///
    /// Returns `Some((value, blinding))` if decryption succeeds.
    ///
    /// **Recommendation**: Use `decrypt_and_verify()` instead, which also
    /// validates the commitment after decryption.
    pub fn decrypt_value(
        &self,
        kyber_sk: &crate::kyber_kem::KyberSecretKey,
    ) -> Option<(u64, [u8; 32])> {
        use pqcrypto_traits::kem::Ciphertext as KemCt;
        
        // Extract components: [nonce(24) | AEAD_ciphertext | Kyber_CT(1088)]
        if self.encrypted_value.len() < 24 + 16 + KYBER768_CT_BYTES {
            return None;
        }
        
        let nonce_bytes = &self.encrypted_value[0..24];
        let ct_end = self.encrypted_value.len() - KYBER768_CT_BYTES;
        let ciphertext = &self.encrypted_value[24..ct_end];
        let kyber_ct_bytes = &self.encrypted_value[ct_end..];
        
        // Kyber decapsulation
        let kyber_ct = crate::kyber_kem::kyber_ct_from_bytes(kyber_ct_bytes).ok()?;
        let ss = crate::kyber_kem::kyber_decapsulate(&kyber_ct, kyber_sk).ok()?;
        
        // Derive AES key (directly from SharedSecret)
        let aes_key = crate::kyber_kem::derive_aes_key_from_shared_secret(&ss, b"TX_VALUE_ENC");
        
        // Decrypt
        use chacha20poly1305::{XChaCha20Poly1305, Key, XNonce, aead::{Aead, KeyInit}};
        let cipher = XChaCha20Poly1305::new(Key::from_slice(&aes_key));
        let nonce = XNonce::from_slice(nonce_bytes);
        
        let plaintext = cipher.decrypt(nonce, ciphertext).ok()?;
        
        if plaintext.len() != 40 {
            return None;
        }
        
        let mut value_bytes = [0u8; 8];
        value_bytes.copy_from_slice(&plaintext[0..8]);
        let value = u64::from_le_bytes(value_bytes);
        
        let mut blinding = [0u8; 32];
        blinding.copy_from_slice(&plaintext[8..40]);
        
        Some((value, blinding))
    }
    
    /// Decrypt + verify commitment (RECOMMENDED API for recipients)
    ///
    /// This is the **safest** way to decrypt an output:
    /// 1. Decrypt (value, blinding) using Kyber secret key
    /// 2. Recompute commitment = SHA3(value || blinding || recipient)
    /// 3. Check recomputed == self.value_commitment
    ///
    /// Returns `Some(value)` only if decryption succeeds AND commitment is valid.
    ///
    /// **Security**: Detects tampering with encrypted_value or commitment fields.
    ///
    /// # Example
    /// ```ignore
    /// let my_value = output.decrypt_and_verify(&my_kyber_sk)?;
    /// // If Some(value), we're guaranteed:
    /// // - Decryption succeeded
    /// // - Commitment is valid
    /// // - No tampering occurred
    /// ```
    pub fn decrypt_and_verify(
        &self,
        kyber_sk: &crate::kyber_kem::KyberSecretKey,
    ) -> Option<u64> {
        // 1. Decrypt
        let (value, blinding) = self.decrypt_value(kyber_sk)?;

        // 2. Recompute commitment
        let mut h = Sha3_256::new();
        h.update(b"TX_OUTPUT_STARK.v1");
        h.update(&value.to_le_bytes());
        h.update(&blinding);
        h.update(&self.recipient);
        let recomputed: Hash32 = h.finalize().into();

        // 3. Verify commitment
        if recomputed == self.value_commitment {
            Some(value) // ✅ Valid!
        } else {
            None // ❌ Tampered data!
        }
    }
}

// =================== Transaction Input ===================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TxInputStark {
    /// Previous output ID (hash)
    pub prev_output_id: Hash32,
    
    /// Index in previous TX
    pub output_index: u32,
    
    /// Spending proof (signature over TX hash)
    pub spending_sig: Vec<u8>, // Falcon512 signature (~698 bytes)
}

// =================== Transaction (STARK) ===================

/// Post-quantum transaction with STARK proofs
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransactionStark {
    /// Inputs (spend previous outputs)
    pub inputs: Vec<TxInputStark>,
    
    /// Outputs (create new outputs with STARK proofs)
    pub outputs: Vec<TxOutputStark>,
    
    /// Transaction fee (plaintext, goes to miner)
    pub fee: u64,
    
    /// Nonce (prevent replay)
    pub nonce: u64,
    
    /// Timestamp (Unix seconds)
    pub timestamp: u64,
}

impl TransactionStark {
    /// Compute transaction ID (hash)
    pub fn id(&self) -> Hash32 {
        let bytes = bincode::serialize(self)
            .expect("TX serialization failed");
        
        let mut h = Sha3_256::new();
        h.update(b"TX_ID.v2");
        h.update(&bytes);
        h.finalize().into()
    }
    
    /// Verify all STARK proofs
    ///
    /// Returns `(valid_count, total_count)`.
    ///
    /// # Performance
    /// ~50ms per output (can be parallelized)
    pub fn verify_all_proofs(&self) -> (u32, u32) {
        let mut valid = 0u32;
        let total = self.outputs.len() as u32;
        
        for output in &self.outputs {
            if output.verify() {
                valid += 1;
            }
        }
        
        (valid, total)
    }
    
    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self)
            .expect("TX serialization failed")
    }
    
    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        bincode::deserialize(bytes)
            .map_err(|e| format!("TX deserialization failed: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kyber_kem::kyber_keypair;
    
    #[test]
    fn test_tx_output_stark_full_flow() {
        let value = 1000u64;
        let mut blinding = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut blinding);
        let recipient = [42u8; 32];
        
        let (kyber_pk, kyber_sk) = kyber_keypair();
        
        // 1. Create output (sender side)
        let output = TxOutputStark::new(value, &blinding, recipient, &kyber_pk);
        
        // 2. Verify STARK proof + commitment binding (anyone can do this)
        assert!(output.verify(), "STARK proof + commitment should be valid");
        
        // 3. Decrypt raw (recipient only)
        let (decrypted_value, decrypted_blinding) = output.decrypt_value(&kyber_sk)
            .expect("Decryption should succeed");
        assert_eq!(decrypted_value, value);
        assert_eq!(decrypted_blinding, blinding);
        
        // 4. Decrypt + verify (RECOMMENDED API)
        let verified_value = output.decrypt_and_verify(&kyber_sk)
            .expect("Decrypt+verify should succeed");
        assert_eq!(verified_value, value);
    }
    
    #[test]
    fn test_commitment_binding_prevents_reuse() {
        let value = 500u64;
        let mut blinding = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut blinding);
        
        let (kyber_pk, _) = kyber_keypair();
        
        // Create two outputs with same value but different recipients
        let recipient1 = [0xAAu8; 32];
        let recipient2 = [0xBBu8; 32];
        
        let output1 = TxOutputStark::new(value, &blinding, recipient1, &kyber_pk);
        let output2 = TxOutputStark::new(value, &blinding, recipient2, &kyber_pk);
        
        // Commitments should be different (recipient is part of commitment)
        assert_ne!(output1.value_commitment, output2.value_commitment);
        
        // Try to swap proofs (attack!)
        let mut output1_tampered = output1.clone();
        output1_tampered.stark_proof = output2.stark_proof.clone();
        
        // Verification should FAIL (commitment mismatch)
        assert!(!output1_tampered.verify(), "Swapped proof should fail verification");
    }
    
    #[test]
    fn test_tampered_commitment_detection() {
        let value = 999u64;
        let mut blinding = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut blinding);
        let recipient = [0x77u8; 32];
        
        let (kyber_pk, kyber_sk) = kyber_keypair();
        
        let output = TxOutputStark::new(value, &blinding, recipient, &kyber_pk);
        
        // Tamper with commitment
        let mut output_tampered = output.clone();
        output_tampered.value_commitment[0] ^= 0xFF; // Flip bits
        
        // STARK proof verification should FAIL (commitment mismatch)
        assert!(!output_tampered.verify(), "Tampered commitment should fail STARK verification");
        
        // Decrypt + verify should also FAIL
        assert!(output_tampered.decrypt_and_verify(&kyber_sk).is_none(),
                "Tampered commitment should fail decrypt_and_verify");
    }
    
    #[test]
    fn test_transaction_stark() {
        let (kyber_pk, _kyber_sk) = kyber_keypair();
        
        let mut blinding1 = [0u8; 32];
        let mut blinding2 = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut blinding1);
        rand::thread_rng().fill_bytes(&mut blinding2);
        
        let output1 = TxOutputStark::new(500, &blinding1, [1u8; 32], &kyber_pk);
        let output2 = TxOutputStark::new(300, &blinding2, [2u8; 32], &kyber_pk);
        
        let tx = TransactionStark {
            inputs: vec![],
            outputs: vec![output1, output2],
            fee: 10,
            nonce: 1,
            timestamp: 1234567890,
        };
        
        // Verify all proofs
        let (valid, total) = tx.verify_all_proofs();
        assert_eq!(valid, 2);
        assert_eq!(total, 2);
        
        // Serialization
        let bytes = tx.to_bytes();
        let tx2 = TransactionStark::from_bytes(&bytes).unwrap();
        assert_eq!(tx.id(), tx2.id());
    }
}
