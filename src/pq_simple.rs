//! Simplified Post-Quantum Crypto for Blockchain
//! Falcon512 + Kyber768 without heavy dependencies

#![forbid(unsafe_code)]

use pqcrypto_falcon::falcon512;
use pqcrypto_kyber::kyber768;
use pqcrypto_traits::sign::{PublicKey, SecretKey, SignedMessage};
use serde::{Deserialize, Serialize};

pub type Hash32 = [u8; 32];

/// Falcon512 Block Signer
pub struct FalconBlockSigner {
    pub_key: falcon512::PublicKey,
    sec_key: falcon512::SecretKey,
}

impl FalconBlockSigner {
    /// Generate new keypair
    pub fn generate() -> Self {
        let (pk, sk) = falcon512::keypair();
        Self {
            pub_key: pk,
            sec_key: sk,
        }
    }
    
    /// Sign block hash
    pub fn sign_block(&self, block_hash: &Hash32) -> Vec<u8> {
        let signed = falcon512::sign(block_hash, &self.sec_key);
        // Extract the signed message bytes
        let bytes = signed.as_bytes();
        bytes.to_vec()
    }
    
    /// Get public key bytes
    pub fn public_key_bytes(&self) -> Vec<u8> {
        self.pub_key.as_bytes().to_vec()
    }
    
    /// Get public key hash (for Node ID)
    pub fn public_key_hash(&self) -> Hash32 {
        use tiny_keccak::{Hasher, Shake};
        let mut sh = Shake::v256();
        sh.update(b"FALCON_PK");
        sh.update(self.pub_key.as_bytes());
        let mut out = [0u8; 32];
        sh.finalize(&mut out);
        out
    }
}

/// Verify Falcon512 block signature
pub fn verify_falcon_signature(
    block_hash: &Hash32,
    signature_bytes: &[u8],
    public_key_bytes: &[u8],
) -> bool {
    // Reconstruct public key
    let pk = match falcon512::PublicKey::from_bytes(public_key_bytes) {
        Ok(key) => key,
        Err(_) => return false,
    };
    
    // Verify signed message
    match falcon512::open(signature_bytes, &pk) {
        Ok(opened_msg) => opened_msg == block_hash,
        Err(_) => false,
    }
}

/// Kyber768 Key Exchange
pub struct KyberKeyExchange {
    pub_key: kyber768::PublicKey,
    sec_key: kyber768::SecretKey,
}

impl KyberKeyExchange {
    /// Generate new keypair
    pub fn generate() -> Self {
        let (pk, sk) = kyber768::keypair();
        Self {
            pub_key: pk,
            sec_key: sk,
        }
    }
    
    /// Encapsulate (generate shared secret)
    pub fn encapsulate(recipient_pk_bytes: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
        let pk = kyber768::PublicKey::from_bytes(recipient_pk_bytes).ok()?;
        let (ss, ct) = kyber768::encapsulate(&pk);
        Some((ss.as_bytes().to_vec(), ct.as_bytes().to_vec()))
    }
    
    /// Decapsulate (receive shared secret)
    pub fn decapsulate(&self, ciphertext_bytes: &[u8]) -> Option<Vec<u8>> {
        let ct = kyber768::Ciphertext::from_bytes(ciphertext_bytes).ok()?;
        let ss = kyber768::decapsulate(&ct, &self.sec_key);
        Some(ss.as_bytes().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_falcon_block_signing() {
        let signer = FalconBlockSigner::generate();
        let block_hash = [0x42u8; 32];
        
        let signature = signer.sign_block(&block_hash);
        let pub_key_bytes = signer.public_key_bytes();
        
        assert!(verify_falcon_signature(&block_hash, &signature, &pub_key_bytes));
        println!("✅ Falcon512 block signing works!");
    }
    
    #[test]
    fn test_kyber_key_exchange() {
        let alice = KyberKeyExchange::generate();
        let bob = KyberKeyExchange::generate();
        
        // Alice encapsulates to Bob
        let bob_pk = bob.pub_key.as_bytes();
        let (alice_ss, ciphertext) = KyberKeyExchange::encapsulate(bob_pk).unwrap();
        
        // Bob decapsulates
        let bob_ss = bob.decapsulate(&ciphertext).unwrap();
        
        assert_eq!(alice_ss, bob_ss);
        println!("✅ Kyber768 key exchange works!");
    }
}
