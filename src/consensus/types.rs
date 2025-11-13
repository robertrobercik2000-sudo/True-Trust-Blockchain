//! Core consensus types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Node identifier (32 bytes - can be pubkey or hash)
pub type NodeId = [u8; 32];

/// Q32.32 fixed-point type for fractional values
pub type Q = u64;
pub type StakeQ = u128;

/// Q32.32 representation of 1.0
pub const ONE_Q: Q = 1u64 << 32;

/// PoT consensus parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PotParams {
    pub epoch_length: u64,       // slots per epoch
    pub slot_duration: u64,      // seconds
    pub lambda_q: Q,             // leader ratio (Q32.32)
    pub min_stake: u128,         // minimum stake to participate
}

impl Default for PotParams {
    fn default() -> Self {
        Self {
            epoch_length: 32,
            slot_duration: 6,
            lambda_q: ONE_Q / 10, // 10% leader ratio
            min_stake: 100_000,
        }
    }
}

/// Trust decay parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustParams {
    pub initial_trust_q: Q,
    pub decay_per_slot: Q,
    pub min_trust_q: Q,
    pub max_trust_q: Q,
}

impl Default for TrustParams {
    fn default() -> Self {
        Self {
            initial_trust_q: ONE_Q,
            decay_per_slot: 100,
            min_trust_q: ONE_Q / 100,
            max_trust_q: 2 * ONE_Q,
        }
    }
}

/// Validator registry entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorInfo {
    pub node_id: NodeId,
    pub stake: StakeQ,
    pub public_key: Vec<u8>,  // Ed25519 or Falcon512
    pub network_addr: String,
}

/// Validator registry
#[derive(Debug, Clone, Default)]
pub struct Registry {
    pub validators: HashMap<NodeId, ValidatorInfo>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, info: ValidatorInfo) {
        self.validators.insert(info.node_id, info);
    }

    pub fn get(&self, node_id: &NodeId) -> Option<&ValidatorInfo> {
        self.validators.get(node_id)
    }

    pub fn total_stake(&self) -> StakeQ {
        self.validators.values().map(|v| v.stake).sum()
    }
}

/// Trust state for all validators
#[derive(Debug, Clone, Default)]
pub struct TrustState {
    pub trust: HashMap<NodeId, Q>,
    pub last_slot: u64,
}

impl TrustState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_trust(&self, node_id: &NodeId) -> Q {
        self.trust.get(node_id).copied().unwrap_or(ONE_Q)
    }

    pub fn set_trust(&mut self, node_id: &NodeId, trust_q: Q) {
        self.trust.insert(*node_id, trust_q);
    }
}

/// Block header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    pub slot: u64,
    pub epoch: u64,
    pub parent_hash: [u8; 32],
    pub state_root: [u8; 32],
    pub txs_root: [u8; 32],
    pub leader: NodeId,
    pub leader_signature: Vec<u8>,
    pub randao_reveal: [u8; 32],
    pub timestamp: u64,
    
    // PoZS zkSNARK proof (optional)
    #[cfg(feature = "zk-proofs")]
    pub zk_proof: Option<Vec<u8>>,
}

/// Transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub from: NodeId,
    pub to: NodeId,
    pub amount: u128,
    pub nonce: u64,
    pub signature: Vec<u8>,
}

/// Block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}

/// RANDAO commit-reveal proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub who: NodeId,
    pub epoch: u64,
    pub commitment: [u8; 32],  // hash(secret)
    pub reveal: Option<[u8; 32]>,  // secret (revealed in epoch)
}

/// Leader witness for eligibility proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderWitness {
    pub who: NodeId,
    pub slot: u64,
    pub epoch: u64,
    pub weights_root: [u8; 32],
    pub stake_q: StakeQ,
    pub trust_q: Q,
    pub merkle_proof: Option<Vec<u8>>,  // classical Merkle proof
    
    #[cfg(feature = "zk-proofs")]
    pub zk_proof: Option<Vec<u8>>,  // zkSNARK proof
}
