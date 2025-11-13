//! Gossip protocol for block propagation

use super::protocol::NetworkMessage;
use super::peer::PeerManager;
use crate::consensus::Block;
use anyhow::Result;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Gossip layer for block propagation
pub struct GossipLayer {
    seen_blocks: Arc<RwLock<HashSet<u64>>>,  // seen slot numbers
    peer_manager: Arc<RwLock<PeerManager>>,
}

impl GossipLayer {
    pub fn new(peer_manager: Arc<RwLock<PeerManager>>) -> Self {
        Self {
            seen_blocks: Arc::new(RwLock::new(HashSet::new())),
            peer_manager,
        }
    }

    /// Gossip new block
    pub async fn gossip_block(&self, block: Block) -> Result<()> {
        let slot = block.header.slot;
        
        // Check if already seen
        {
            let mut seen = self.seen_blocks.write().await;
            if seen.contains(&slot) {
                return Ok(());
            }
            seen.insert(slot);
        }

        info!("Gossiping block at slot {}", slot);

        // Broadcast to peers
        let pm = self.peer_manager.read().await;
        pm.broadcast(NetworkMessage::NewBlock(block)).await?;

        Ok(())
    }

    /// Mark block as seen
    pub async fn mark_seen(&self, slot: u64) {
        self.seen_blocks.write().await.insert(slot);
    }
}
