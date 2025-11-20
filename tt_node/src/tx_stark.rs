#![forbid(unsafe_code)]

//! Post-Quantum Transactions with STARK Range Proofs (Winterfell 0.13)
//!
//! Używa:
//!   - Kyber768 dla szyfrowania wartości outputu,
//!   - XChaCha20-Poly1305 dla AEAD,
//!   - STARK range-proof (0 <= value < 2^64) z `stark_full.rs`.
//!
//! Format jest kompatybilny z wcześniejszą wersją:
//!   - TxOutputStark::stark_proof = bincode(StarkRangeProof)
//!   - TxOutputStark::verify() → używa STARKVerifier::verify(&StarkRangeProof)

use serde::{Serialize, Deserialize};
use sha3::{Sha3_256, Digest};
use rand::RngCore;

use crate::core::Hash32;
use crate::stark_full::{STARKProver, STARKVerifier, StarkRangeProof};

/// Kyber768 ciphertext size (1088 bytes)
const KYBER768_CT_BYTES: usize = 1088;

/// Output z zakodowaną wartością (zaszyfrowaną) i STARK range-proof
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TxOutputStark {
    /// Commitment na wartość: H("TX_OUTPUT_STARK.v1" || value || blinding || recipient)
    pub value_commitment: Hash32,

    /// STARK proof bytes (bincode(StarkRangeProof))
    pub stark_proof: Vec<u8>,

    /// Odbiorca (np. hash klucza publicznego)
    pub recipient: Hash32,

    /// Zaszyfrowana wartość: nonce(24B) || AEAD || KyberCiphertext(1088B)
    pub encrypted_value: Vec<u8>,
}

impl TxOutputStark {
    /// Tworzy nowy output:
    /// - generuje commitment,
    /// - tworzy STARK range-proof na value,
    /// - szyfruje (value, blinding) pod Kyber PK odbiorcy.
    pub fn new(
        value: u64,
        blinding: &[u8; 32],
        recipient: Hash32,
        recipient_kyber_pk: &crate::kyber_kem::KyberPublicKey,
    ) -> Self {
        // commitment = SHA3("TX_OUTPUT_STARK.v1" || value || blinding || recipient)
        let mut h = Sha3_256::new();
        h.update(b"TX_OUTPUT_STARK.v1");
        h.update(&value.to_le_bytes());
        h.update(blinding);
        h.update(&recipient);
        let commitment: Hash32 = h.finalize().into();

        // STARK range proof (0 <= value < 2^64)
        let proof = STARKProver::prove_range_with_commitment(value, &commitment);
        let stark_proof = bincode::serialize(&proof).expect("serialize STARK proof");

        // Kyber encapsulation + AEAD (XChaCha20-Poly1305)
        let (ss, ct) = crate::kyber_kem::kyber_encapsulate(recipient_kyber_pk);
        let ss_bytes = crate::kyber_kem::kyber_ss_to_bytes(&ss);
        let aes_key = crate::kyber_kem::derive_aes_key_from_shared_secret_bytes(
            &ss_bytes,
            b"TX_VALUE_ENC",
        );

        use chacha20poly1305::{
            XChaCha20Poly1305, Key, XNonce,
            aead::{Aead, KeyInit},
        };

        let cipher = XChaCha20Poly1305::new(Key::from_slice(&aes_key));
        let mut nonce_bytes = [0u8; 24];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = XNonce::from(nonce_bytes);

        // plaintext = value(8B) || blinding(32B)
        let mut plaintext = Vec::with_capacity(8 + 32);
        plaintext.extend_from_slice(&value.to_le_bytes());
        plaintext.extend_from_slice(blinding);

        let ciphertext = cipher.encrypt(&nonce, plaintext.as_ref())
            .expect("encryption failed");

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

    /// Weryfikuje tylko STARK range proof (zakres value).
    ///
    /// Związek z commitmentem jest sprawdzany w `decrypt_and_verify`.
    pub fn verify(&self) -> bool {
        if let Ok(proof) = bincode::deserialize::<StarkRangeProof>(&self.stark_proof) {
            STARKVerifier::verify(&proof)
        } else {
            false
        }
    }

    /// Odszyfrowuje (value, blinding) z encrypted_value, używając Kyber SK odbiorcy.
    pub fn decrypt_value(
        &self,
        kyber_sk: &crate::kyber_kem::KyberSecretKey,
    ) -> Option<(u64, [u8; 32])> {
        if self.encrypted_value.len() < 24 + 16 + KYBER768_CT_BYTES {
            return None;
        }

        let nonce_bytes = &self.encrypted_value[0..24];
        let ct_end = self.encrypted_value.len() - KYBER768_CT_BYTES;
        let ciphertext = &self.encrypted_value[24..ct_end];
        let kyber_ct_bytes = &self.encrypted_value[ct_end..];

        let kyber_ct = crate::kyber_kem::kyber_ct_from_bytes(kyber_ct_bytes).ok()?;
        let ss = crate::kyber_kem::kyber_decapsulate(&kyber_ct, kyber_sk).ok()?;
        let aes_key = crate::kyber_kem::derive_aes_key_from_shared_secret(
            &ss,
            b"TX_VALUE_ENC",
        );

        use chacha20poly1305::{
            XChaCha20Poly1305, Key, XNonce,
            aead::{Aead, KeyInit},
        };

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

    /// Odszyfrowuje value i dodatkowo sprawdza, że commitment się zgadza:
    ///
    ///   recomputed = H("TX_OUTPUT_STARK.v1" || value || blinding || recipient)
    ///   recomputed == self.value_commitment ?
    pub fn decrypt_and_verify(
        &self,
        kyber_sk: &crate::kyber_kem::KyberSecretKey,
    ) -> Option<u64> {
        let (value, blinding) = self.decrypt_value(kyber_sk)?;

        let mut h = Sha3_256::new();
        h.update(b"TX_OUTPUT_STARK.v1");
        h.update(&value.to_le_bytes());
        h.update(&blinding);
        h.update(&self.recipient);
        let recomputed: Hash32 = h.finalize().into();

        if recomputed == self.value_commitment {
            Some(value)
        } else {
            None
        }
    }
}

/// Wejście transakcji – powiązanie z poprzednim outputem
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TxInputStark {
    pub prev_output_id: Hash32,
    pub output_index: u32,
    /// Sygnatura Falcon512 (spend authorization) – do zdefiniowania w wyższej warstwie
    pub spending_sig: Vec<u8>,
}

/// Cała transakcja z STARK-owymi outputami
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransactionStark {
    pub inputs: Vec<TxInputStark>,
    pub outputs: Vec<TxOutputStark>,
    pub fee: u64,
    pub nonce: u64,
    pub timestamp: u64,
}

impl TransactionStark {
    /// ID transakcji:
    ///   TX_ID.v2 || bincode(self) → SHA3-256
    pub fn id(&self) -> Hash32 {
        let bytes = bincode::serialize(self).expect("tx serialize");
        let mut h = Sha3_256::new();
        h.update(b"TX_ID.v2");
        h.update(&bytes);
        h.finalize().into()
    }

    /// Weryfikuje wszystkie STARK proofy w outputs.
    /// Zwraca (ile_poprawnych, ile_łącznie).
    pub fn verify_all_proofs(&self) -> (u32, u32) {
        let mut valid = 0u32;
        for o in &self.outputs {
            if o.verify() {
                valid += 1;
            }
        }
        (valid, self.outputs.len() as u32)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).expect("tx serialize")
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        bincode::deserialize(bytes)
            .map_err(|e| format!("TX deserialization failed: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;

    #[test]
    fn test_tx_output_stark_new_and_verify() {
        let (kyber_pk, _) = crate::kyber_kem::kyber_keypair();
        let mut blinding = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut blinding);

        let output = TxOutputStark::new(
            100_000,
            &blinding,
            [1u8; 32],
            &kyber_pk,
        );

        // STARK proof powinien istnieć
        assert!(!output.stark_proof.is_empty());

        // STARK proof powinien się weryfikować
        assert!(output.verify());
    }

    #[test]
    fn test_transaction_stark_proofs() {
        let (kyber_pk, _) = crate::kyber_kem::kyber_keypair();
        let mut b1 = [0u8; 32];
        let mut b2 = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut b1);
        rand::thread_rng().fill_bytes(&mut b2);

        let o1 = TxOutputStark::new(123, &b1, [1u8; 32], &kyber_pk);
        let o2 = TxOutputStark::new(456, &b2, [2u8; 32], &kyber_pk);

        let tx = TransactionStark {
            inputs: vec![],
            outputs: vec![o1, o2],
            fee: 10,
            nonce: 1,
            timestamp: 1_234_567_890,
        };

        let (valid, total) = tx.verify_all_proofs();
        assert_eq!(valid, total);
        assert_eq!(total, 2);
    }
}
