//! Core node functionality

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::chain_store::ChainStore;
use crate::consensus_pro::ConsensusPro;

/// Core blockchain node implementation
pub struct NodeCore {
    /// Chain data store
    pub chain_store: Arc<RwLock<ChainStore>>,
    
    /// Consensus engine
    pub consensus: Arc<RwLock<ConsensusPro>>,
    
    /// Node configuration
    pub config: NodeConfig,
    
    /// Is this a validator node?
    pub is_validator: bool,
}

/// Node configuration
#[derive(Clone, Debug)]
pub struct NodeConfig {
    /// Data directory
    pub data_dir: PathBuf,
    
    /// Chain ID
    pub chain_id: String,
    
    /// Block time in milliseconds
    pub block_time_ms: u64,
}

impl NodeCore {
    /// Create a new node instance
    pub fn new(data_dir: PathBuf, is_validator: bool) -> Result<Self> {
        // Create data directory if it doesn't exist
        std::fs::create_dir_all(&data_dir)
            .context("Failed to create data directory")?;
        
        // Initialize chain store
        let chain_store = Arc::new(RwLock::new(
            ChainStore::new()
        ));
        
        // Initialize consensus
        let consensus = Arc::new(RwLock::new(
            ConsensusPro::new_default()
        ));
        
        // Default configuration
        let config = NodeConfig {
            data_dir,
            chain_id: "tt-mainnet".to_string(),
            block_time_ms: 6000, // 6 seconds
        };
        
        Ok(Self {
            chain_store,
            consensus,
            config,
            is_validator,
        })
    }
    
    /// Initialize from genesis
    pub async fn init_genesis(&self, genesis_data: &[u8]) -> Result<()> {
        // TODO: Parse and apply genesis state
        Ok(())
    }
    
    /// Start the node
    pub async fn start(&self) -> Result<()> {
        if self.is_validator {
            println!("Starting as validator node...");
            // TODO: Start block production
        } else {
            println!("Starting as full node...");
            // TODO: Start syncing
        }
        
        Ok(())
    }
    
    /// Stop the node
    pub async fn stop(&self) -> Result<()> {
        println!("Stopping node...");
        // TODO: Graceful shutdown
        Ok(())
    }
}
