#![forbid(unsafe_code)]

//! Transaction types and parsing for TRUE TRUST blockchain

use serde::{Deserialize, Serialize};
use crate::core::Hash32;

/// Transaction input (spending a previous output)
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TxInput {
    /// Nullifier (prevents double-spend)
    pub nullifier: Hash32,
    /// Reference to note being spent (optional, for transparency)
    pub note_ref: Option<Hash32>,
}

/// Transaction output (creating new note)
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TxOutput {
    /// Pedersen commitment: C = r·G + v·H
    pub commitment: Vec<u8>,  // 32 bytes (RistrettoPoint)
    /// Bulletproof range proof (proves v ∈ [0, 2^64))
    pub bulletproof: Vec<u8>, // ~672 bytes for 64-bit
    /// Ephemeral public key for stealth address
    pub eph_pub: Vec<u8>,     // 32 bytes
    /// Encrypted hint for receiver (keysearch)
    pub enc_hint: Vec<u8>,    // Variable size
    /// Bloom filter tag (16-bit) for fast pre-filtering
    pub filter_tag16: u16,
}

/// Complete transaction
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Transaction {
    /// Inputs (nullifiers)
    pub inputs: Vec<TxInput>,
    /// Outputs (commitments + Bulletproofs)
    pub outputs: Vec<TxOutput>,
    /// Transaction fee (in smallest unit)
    pub fee: u64,
    /// Sender's nonce (for replay protection)
    pub nonce: u64,
    /// Signature (Ed25519 or Falcon512)
    pub signature: Vec<u8>,
    /// Optional: RISC0 receipt for private claim
    pub risc0_receipt: Vec<u8>,
}

impl Transaction {
    /// Parse transaction from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(data)
    }
    
    /// Serialize transaction to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(self)
    }
    
    /// Compute transaction hash
    pub fn hash(&self) -> Hash32 {
        use crate::core::shake256_bytes;
        let bytes = self.to_bytes().unwrap_or_default();
        shake256_bytes(&bytes)
    }
    
    /// Verify all Bulletproofs in this transaction
    /// Returns (total_count, valid_count)
    pub fn verify_bulletproofs(&self) -> (u32, u32) {
        use crate::bp::{parse_dalek_range_proof_64, verify_range_proof_64};
        
        let mut total = 0u32;
        let mut valid = 0u32;
        
        for output in &self.outputs {
            total += 1;
            
            // Parse commitment (C = r·G + v·H)
            let commitment_bytes = if output.commitment.len() >= 32 {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&output.commitment[..32]);
                arr
            } else {
                continue; // Invalid commitment
            };
            
            // Parse Bulletproof
            let proof = match parse_dalek_range_proof_64(&output.bulletproof) {
                Ok(p) => p,
                Err(_) => continue,
            };
            
            // Verify proof
            // Note: verify_range_proof_64 expects commitment as first bytes, H_pedersen as second
            let H = crate::bp::derive_H_pedersen();
            if verify_range_proof_64(&proof, commitment_bytes, H).is_ok() {
                valid += 1;
            }
        }
        
        (total, valid)
    }
    
    /// Estimate size in bytes
    pub fn estimated_size(&self) -> usize {
        self.to_bytes().map(|b| b.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tx_serialization() {
        let tx = Transaction {
            inputs: vec![],
            outputs: vec![],
            fee: 100,
            nonce: 1,
            signature: vec![0u8; 64],
            risc0_receipt: vec![],
        };
        
        let bytes = tx.to_bytes().unwrap();
        let tx2 = Transaction::from_bytes(&bytes).unwrap();
        assert_eq!(tx.fee, tx2.fee);
        assert_eq!(tx.nonce, tx2.nonce);
    }
}
