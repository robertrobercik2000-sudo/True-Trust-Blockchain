#![forbid(unsafe_code)]

//! Core blockchain primitives (Hash32, Block, timestamp utilities)

use serde::{Deserialize, Serialize};

pub type Hash32 = [u8; 32];

/// Fast SHAKE256 hash
pub fn shake256_bytes(data: &[u8]) -> Hash32 {
    use tiny_keccak::{Shake, Hasher};
    let mut sh = Shake::v256();
    sh.update(data);
    let mut out = [0u8; 32];
    sh.finalize(&mut out);
    out
}

pub fn bytes32(data: &[u8]) -> Hash32 {
    shake256_bytes(data)
}

pub fn now_ts() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct BlockHeader {
    pub parent: Hash32,
    pub height: u64,
    pub author_pk: Vec<u8>,
    pub author_pk_hash: Hash32,
    pub task_seed: Hash32,
    pub timestamp: u64,
    pub cum_weight_hint: f64,
    pub parent_state_hash: Hash32,
    pub result_state_hash: Hash32,
}

impl BlockHeader {
    pub fn id(&self) -> Hash32 {
        let bin = bincode::serialize(self).expect("hdr->bin");
        shake256_bytes(&bin)
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Block {
    pub header: BlockHeader,
    pub author_sig: Vec<u8>,
    pub zk_receipt_bincode: Vec<u8>,
    pub transactions: Vec<u8>, // placeholder - use actual Tx type
}
