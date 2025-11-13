//! Blockchain storage layer (sled embedded DB)

use crate::consensus::{Block, EpochSnapshot};
use anyhow::{Context, Result};
use sled::Db;
use std::path::Path;

/// Blockchain storage
pub struct BlockchainStorage {
    db: Db,
}

impl BlockchainStorage {
    /// Open storage at path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let db = sled::open(path)
            .context("Failed to open database")?;
        Ok(Self { db })
    }

    /// Store block
    pub fn store_block(&self, block: &Block) -> Result<()> {
        let key = format!("block:{}", block.header.slot);
        let data = bincode::serialize(block)?;
        self.db.insert(key.as_bytes(), data)?;
        self.db.flush()?;
        Ok(())
    }

    /// Get block by slot
    pub fn get_block(&self, slot: u64) -> Result<Option<Block>> {
        let key = format!("block:{}", slot);
        if let Some(data) = self.db.get(key.as_bytes())? {
            let block: Block = bincode::deserialize(&data)?;
            Ok(Some(block))
        } else {
            Ok(None)
        }
    }

    /// Get latest block
    pub fn latest_block(&self) -> Result<Option<Block>> {
        let prefix = b"block:";
        let mut iter = self.db.scan_prefix(prefix).rev();
        
        if let Some(result) = iter.next() {
            let (_key, data) = result?;
            let block: Block = bincode::deserialize(&data)?;
            Ok(Some(block))
        } else {
            Ok(None)
        }
    }

    /// Store epoch snapshot
    pub fn store_snapshot(&self, snapshot: &EpochSnapshot) -> Result<()> {
        let key = format!("snapshot:{}", snapshot.epoch);
        let data = bincode::serialize(snapshot)?;
        self.db.insert(key.as_bytes(), data)?;
        self.db.flush()?;
        Ok(())
    }

    /// Get epoch snapshot
    pub fn get_snapshot(&self, epoch: u64) -> Result<Option<EpochSnapshot>> {
        let key = format!("snapshot:{}", epoch);
        if let Some(data) = self.db.get(key.as_bytes())? {
            let snapshot: EpochSnapshot = bincode::deserialize(&data)?;
            Ok(Some(snapshot))
        } else {
            Ok(None)
        }
    }
}
