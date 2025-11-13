//! Network protocol messages

use crate::consensus::{Block, BlockHeader, Proposal};
use serde::{Deserialize, Serialize};
use tokio_util::codec::{Decoder, Encoder, LengthDelimitedCodec};
use bytes::{Buf, BufMut, BytesMut};
use std::io;

/// Network message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkMessage {
    // Consensus
    NewBlock(Block),
    BlockRequest(u64),  // slot number
    BlockResponse(Option<Block>),
    
    // RANDAO
    RandaoCommit(Proposal),
    RandaoReveal(Proposal),
    
    // P2P
    Ping,
    Pong,
    PeerList(Vec<String>),
    
    // Sync
    SyncRequest { start_slot: u64, end_slot: u64 },
    SyncResponse { blocks: Vec<Block> },
}

/// Message codec (length-prefixed bincode)
pub struct MessageCodec {
    inner: LengthDelimitedCodec,
}

impl MessageCodec {
    pub fn new() -> Self {
        Self {
            inner: LengthDelimitedCodec::builder()
                .max_frame_length(16 * 1024 * 1024) // 16 MB max
                .new_codec(),
        }
    }
}

impl Default for MessageCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder for MessageCodec {
    type Item = NetworkMessage;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if let Some(frame) = self.inner.decode(src)? {
            let msg: NetworkMessage = bincode::deserialize(&frame)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            Ok(Some(msg))
        } else {
            Ok(None)
        }
    }
}

impl Encoder<NetworkMessage> for MessageCodec {
    type Error = io::Error;

    fn encode(&mut self, item: NetworkMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let data = bincode::serialize(&item)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        
        self.inner.encode(bytes::Bytes::from(data), dst)?;
        Ok(())
    }
}
