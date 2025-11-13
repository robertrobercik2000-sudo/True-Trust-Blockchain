//! TRUE_TRUST Blockchain Node Library
//! 
//! Complete blockchain implementation combining:
//! - PoT (Proof-of-Trust) consensus with RANDAO beacon
//! - PoZS (Proof-of-ZK-Shares) zkSNARK layer
//! - Post-quantum wallet (Falcon512 + ML-KEM)
//! - P2P networking and block propagation

#![forbid(unsafe_code)]

pub mod crypto;
pub mod consensus;
pub mod wallet;
pub mod storage;
pub mod network;
pub mod node;

#[cfg(feature = "zk-proofs")]
pub mod zk;

// Re-export main types
pub use consensus::{
    Block, BlockHeader, EpochSnapshot, NodeId, PotParams, Proposal, 
    RandaoBeacon, Registry, TrustParams, TrustState, Q, ONE_Q,
};
pub use wallet::{PqWallet, WalletConfig, WalletKeys};
pub use node::{BlockchainNode, NodeConfig};
