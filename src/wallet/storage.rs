//! Wallet encrypted storage

use crate::wallet::keys::{KeyExport, WalletKeys};
use anyhow::{ensure, Context, Result};
use argon2::{Argon2, Params, Version};
use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    XChaCha20Poly1305, XNonce,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use zeroize::Zeroizing;

/// Encrypted wallet file format
#[derive(Serialize, Deserialize)]
pub struct EncryptedWallet {
    pub version: u32,
    pub salt: Vec<u8>,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

impl EncryptedWallet {
    /// Encrypt wallet keys with password
    pub fn encrypt(keys: &WalletKeys, password: &str) -> Result<Self> {
        // Generate salt
        let mut salt = vec![0u8; 32];
        OsRng.fill_bytes(&mut salt);

        // Derive key with Argon2id
        let key = Self::derive_key(password.as_bytes(), &salt)?;

        // Serialize keys
        let export: KeyExport = keys.into();
        let plaintext = bincode::serialize(&export)
            .context("Failed to serialize keys")?;

        // Encrypt with XChaCha20-Poly1305
        let cipher = XChaCha20Poly1305::new((&*key).into());
        let mut nonce_bytes = vec![0u8; 24];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = XNonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_ref())
            .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;

        Ok(Self {
            version: 1,
            salt,
            nonce: nonce_bytes,
            ciphertext,
        })
    }

    /// Decrypt wallet keys with password
    pub fn decrypt(&self, password: &str) -> Result<WalletKeys> {
        ensure!(self.version == 1, "Unsupported wallet version");

        // Derive key
        let key = Self::derive_key(password.as_bytes(), &self.salt)?;

        // Decrypt
        let cipher = XChaCha20Poly1305::new((&*key).into());
        let nonce = XNonce::from_slice(&self.nonce);

        let plaintext = cipher
            .decrypt(nonce, self.ciphertext.as_ref())
            .map_err(|_| anyhow::anyhow!("Decryption failed - wrong password?"))?;

        // Deserialize
        let export: KeyExport = bincode::deserialize(&plaintext)
            .context("Failed to deserialize keys")?;

        Ok(export.into())
    }

    /// Save to file
    pub fn save(&self, path: &Path) -> Result<()> {
        let data = bincode::serialize(self)
            .context("Failed to serialize wallet")?;
        fs::write(path, data)
            .with_context(|| format!("Failed to write wallet to {:?}", path))?;
        Ok(())
    }

    /// Load from file
    pub fn load(path: &Path) -> Result<Self> {
        let data = fs::read(path)
            .with_context(|| format!("Failed to read wallet from {:?}", path))?;
        let wallet: Self = bincode::deserialize(&data)
            .context("Failed to deserialize wallet")?;
        Ok(wallet)
    }

    fn derive_key(password: &[u8], salt: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
        let params = Params::new(65536, 3, 1, Some(32))
            .map_err(|e| anyhow::anyhow!("Argon2 params: {}", e))?;
        let argon2 = Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, params);

        let mut key = Zeroizing::new([0u8; 32]);
        argon2
            .hash_password_into(password, salt, &mut *key)
            .map_err(|e| anyhow::anyhow!("Key derivation failed: {}", e))?;

        Ok(key)
    }
}
