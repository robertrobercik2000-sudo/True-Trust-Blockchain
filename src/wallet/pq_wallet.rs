//! Post-quantum wallet implementation

use super::keys::WalletKeys;
use super::storage::EncryptedWallet;
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Wallet configuration
#[derive(Debug, Clone)]
pub struct WalletConfig {
    pub wallet_dir: PathBuf,
}

impl Default for WalletConfig {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            wallet_dir: home.join(".tt_wallet"),
        }
    }
}

/// Post-quantum wallet
pub struct PqWallet {
    pub config: WalletConfig,
    pub keys: Option<WalletKeys>,
}

impl PqWallet {
    pub fn new(config: WalletConfig) -> Self {
        Self {
            config,
            keys: None,
        }
    }

    /// Create new wallet
    pub fn create(&mut self, password: &str) -> Result<()> {
        let keys = WalletKeys::generate();
        
        // Save encrypted wallet
        std::fs::create_dir_all(&self.config.wallet_dir)?;
        let path = self.wallet_file();
        
        let encrypted = EncryptedWallet::encrypt(&keys, password)?;
        encrypted.save(&path)?;

        self.keys = Some(keys);
        Ok(())
    }

    /// Unlock wallet
    pub fn unlock(&mut self, password: &str) -> Result<()> {
        let path = self.wallet_file();
        let encrypted = EncryptedWallet::load(&path)?;
        let keys = encrypted.decrypt(password)?;
        
        self.keys = Some(keys);
        Ok(())
    }

    /// Lock wallet (clear keys from memory)
    pub fn lock(&mut self) {
        self.keys = None;
    }

    /// Check if wallet file exists
    pub fn exists(&self) -> bool {
        self.wallet_file().exists()
    }

    /// Get wallet file path
    fn wallet_file(&self) -> PathBuf {
        self.config.wallet_dir.join("wallet.enc")
    }

    /// Get keys (requires unlocked wallet)
    pub fn keys(&self) -> Result<&WalletKeys> {
        self.keys.as_ref().ok_or_else(|| anyhow::anyhow!("Wallet is locked"))
    }

    /// Get node ID
    pub fn node_id(&self) -> Result<[u8; 32]> {
        Ok(self.keys()?.node_id())
    }

    /// Sign message
    pub fn sign(&self, msg: &[u8]) -> Result<Vec<u8>> {
        self.keys()?.sign(msg)
    }
}
