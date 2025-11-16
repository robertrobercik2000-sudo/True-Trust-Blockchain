#![forbid(unsafe_code)]

use serde::{Serialize, Deserialize};
use sha3::{Sha3_256, Digest};
use rand::RngCore;

use crate::core::Hash32;
use crate::zk::range_stark_winterfell::{
    prove_range_bound,
    verify_range_bound_with_commitment,
};

/// Kyber768 ciphertext size (1088 bytes)
const KYBER768_CT_BYTES: usize = 1088;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TxOutputStark {
    pub value_commitment: Hash32,
    pub stark_proof: Vec<u8>,       // WrappedProof bytes
    pub recipient: Hash32,
    pub encrypted_value: Vec<u8>,   // nonce||AEAD||KyberCT
}

impl TxOutputStark {
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

        // STARK (Winterfell) – proof with binding
        let stark_proof = prove_range_bound(value, commitment, None);

        // Kyber encapsulation + AEAD
        let (ss, ct) = crate::kyber_kem::kyber_encapsulate(recipient_kyber_pk);
        let ss_bytes = crate::kyber_kem::kyber_ss_to_bytes(&ss);
        let aes_key = crate::kyber_kem::derive_aes_key_from_shared_secret_bytes(&ss_bytes, b"TX_VALUE_ENC");

        use chacha20poly1305::{XChaCha20Poly1305, Key, XNonce, aead::{Aead, KeyInit}};
        let cipher = XChaCha20Poly1305::new(Key::from_slice(&aes_key));
        let mut nonce_bytes = [0u8; 24];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = XNonce::from(nonce_bytes);

        let mut plaintext = Vec::with_capacity(8 + 32);
        plaintext.extend_from_slice(&value.to_le_bytes());
        plaintext.extend_from_slice(blinding);
        let ciphertext = cipher.encrypt(&nonce, plaintext.as_ref()).expect("encryption failed");

        let ct_bytes = crate::kyber_kem::kyber_ct_to_bytes(&ct);
        let mut encrypted_value = Vec::with_capacity(24 + ciphertext.len() + ct_bytes.len());
        encrypted_value.extend_from_slice(&nonce_bytes);
        encrypted_value.extend_from_slice(&ciphertext);
        encrypted_value.extend_from_slice(ct_bytes);

        Self { value_commitment: commitment, stark_proof, recipient, encrypted_value }
    }

    pub fn verify(&self) -> bool {
        verify_range_bound_with_commitment(&self.stark_proof, self.value_commitment)
    }

    pub fn decrypt_value(&self, kyber_sk: &crate::kyber_kem::KyberSecretKey) -> Option<(u64, [u8; 32])> {
        use pqcrypto_traits::kem::Ciphertext as KemCt;

        if self.encrypted_value.len() < 24 + 16 + KYBER768_CT_BYTES { return None; }
        let nonce_bytes = &self.encrypted_value[0..24];
        let ct_end = self.encrypted_value.len() - KYBER768_CT_BYTES;
        let ciphertext = &self.encrypted_value[24..ct_end];
        let kyber_ct_bytes = &self.encrypted_value[ct_end..];

        let kyber_ct = crate::kyber_kem::kyber_ct_from_bytes(kyber_ct_bytes).ok()?;
        let ss = crate::kyber_kem::kyber_decapsulate(&kyber_ct, kyber_sk).ok()?;
        let aes_key = crate::kyber_kem::derive_aes_key_from_shared_secret(&ss, b"TX_VALUE_ENC");

        use chacha20poly1305::{XChaCha20Poly1305, Key, XNonce, aead::{Aead, KeyInit}};
        let cipher = XChaCha20Poly1305::new(Key::from_slice(&aes_key));
        let nonce = XNonce::from_slice(nonce_bytes);
        let plaintext = cipher.decrypt(nonce, ciphertext).ok()?;
        if plaintext.len() != 40 { return None; }

        let mut value_bytes = [0u8; 8];
        value_bytes.copy_from_slice(&plaintext[0..8]);
        let value = u64::from_le_bytes(value_bytes);

        let mut blinding = [0u8; 32];
        blinding.copy_from_slice(&plaintext[8..40]);
        Some((value, blinding))
    }

    pub fn decrypt_and_verify(&self, kyber_sk: &crate::kyber_kem::KyberSecretKey) -> Option<u64> {
        let (value, blinding) = self.decrypt_value(kyber_sk)?;
        let mut h = Sha3_256::new();
        h.update(b"TX_OUTPUT_STARK.v1");
        h.update(&value.to_le_bytes());
        h.update(&blinding);
        h.update(&self.recipient);
        let recomputed: Hash32 = h.finalize().into();
        if recomputed == self.value_commitment { Some(value) } else { None }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TxInputStark {
    pub prev_output_id: Hash32,
    pub output_index: u32,
    pub spending_sig: Vec<u8>, // Falcon512
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransactionStark {
    pub inputs: Vec<TxInputStark>,
    pub outputs: Vec<TxOutputStark>,
    pub fee: u64,
    pub nonce: u64,
    pub timestamp: u64,
}

impl TransactionStark {
    pub fn id(&self) -> Hash32 {
        let bytes = bincode::serialize(self).expect("tx serialize");
        let mut h = Sha3_256::new();
        h.update(b"TX_ID.v2");
        h.update(&bytes);
        h.finalize().into()
    }

    pub fn verify_all_proofs(&self) -> (u32, u32) {
        let mut valid = 0u32;
        for o in &self.outputs {
            if o.verify() { valid += 1; }
        }
        (valid, self.outputs.len() as u32)
    }

    pub fn to_bytes(&self) -> Vec<u8> { bincode::serialize(self).expect("tx serialize") }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        bincode::deserialize(bytes).map_err(|e| format!("TX deserialization failed: {}", e))
    }
}
