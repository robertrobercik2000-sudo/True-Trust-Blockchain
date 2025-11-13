//! Wallet key management - Falcon512 + ML-KEM

use pqcrypto_falcon::falcon512;
use pqcrypto_kyber::kyber768;
use pqcrypto_traits::sign::{PublicKey as _, SecretKey as _, DetachedSignature as _};
use pqcrypto_traits::kem::{PublicKey as KemPk, SecretKey as KemSk};
use zeroize::Zeroizing;
use serde::{Deserialize, Serialize};

/// Wallet keys (Falcon512 for signing + Kyber768 for KEM)
#[derive(Clone)]
pub struct WalletKeys {
    pub falcon_pk: Vec<u8>,
    pub falcon_sk: Zeroizing<Vec<u8>>,
    pub kyber_pk: Vec<u8>,
    pub kyber_sk: Zeroizing<Vec<u8>>,
}

impl WalletKeys {
    /// Generate new keypair
    pub fn generate() -> Self {
        let (falcon_pk, falcon_sk) = falcon512::keypair();
        let (kyber_pk, kyber_sk) = kyber768::keypair();

        Self {
            falcon_pk: falcon_pk.as_bytes().to_vec(),
            falcon_sk: Zeroizing::new(falcon_sk.as_bytes().to_vec()),
            kyber_pk: kyber_pk.as_bytes().to_vec(),
            kyber_sk: Zeroizing::new(kyber_sk.as_bytes().to_vec()),
        }
    }

    /// Sign message with Falcon512
    pub fn sign(&self, msg: &[u8]) -> anyhow::Result<Vec<u8>> {
        let sk = falcon512::SecretKey::from_bytes(&self.falcon_sk)
            .map_err(|e| anyhow::anyhow!("Invalid secret key: {:?}", e))?;
        let sig = falcon512::detached_sign(msg, &sk);
        Ok(sig.as_bytes().to_vec())
    }

    /// Verify signature
    pub fn verify(pk_bytes: &[u8], msg: &[u8], sig_bytes: &[u8]) -> anyhow::Result<bool> {
        let pk = falcon512::PublicKey::from_bytes(pk_bytes)
            .map_err(|e| anyhow::anyhow!("Invalid public key: {:?}", e))?;
        let sig = falcon512::DetachedSignature::from_bytes(sig_bytes)
            .map_err(|e| anyhow::anyhow!("Invalid signature: {:?}", e))?;
        
        match falcon512::verify_detached_signature(&sig, msg, &pk) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Derive NodeId from public key (hash of falcon_pk)
    pub fn node_id(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(b"NODE_ID.v1");
        h.update(&self.falcon_pk);
        let out = h.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&out);
        id
    }
}

/// Serializable key export
#[derive(Serialize, Deserialize)]
pub struct KeyExport {
    pub falcon_pk: Vec<u8>,
    pub falcon_sk: Vec<u8>,
    pub kyber_pk: Vec<u8>,
    pub kyber_sk: Vec<u8>,
}

impl From<&WalletKeys> for KeyExport {
    fn from(keys: &WalletKeys) -> Self {
        Self {
            falcon_pk: keys.falcon_pk.clone(),
            falcon_sk: keys.falcon_sk.to_vec(),
            kyber_pk: keys.kyber_pk.clone(),
            kyber_sk: keys.kyber_sk.to_vec(),
        }
    }
}

impl From<KeyExport> for WalletKeys {
    fn from(export: KeyExport) -> Self {
        Self {
            falcon_pk: export.falcon_pk,
            falcon_sk: Zeroizing::new(export.falcon_sk),
            kyber_pk: export.kyber_pk,
            kyber_sk: Zeroizing::new(export.kyber_sk),
        }
    }
}
