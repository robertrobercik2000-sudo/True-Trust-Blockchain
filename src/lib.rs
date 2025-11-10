//! TRUE_TRUST Proof-of-Trust consensus module
//!
//! This module implements a Proof-of-Trust consensus mechanism with:
//! - Trust decay/reward system
//! - RANDAO commit-reveal beacon
//! - Merkle tree-based weight snapshots
//! - Sortition-based leader selection
//! - Equivocation detection and slashing

pub mod crypto;
pub mod crypto_kmac_consensus;
pub mod pot;
pub mod pot_node;
pub mod snapshot;

// Re-export main types for convenience
pub use pot::{
    detect_equivocation, finalize_epoch_and_slash, q_from_basis_points, q_from_ratio,
    q_from_ratio128, slash_equivocation, verify_leader_and_update_trust,
    verify_leader_with_witness, EpochSnapshot, LeaderWitness, MerkleProof, NodeId, PotParams,
    RandaoBeacon, Registry, TrustParams, TrustState, ONE_Q, Q,
};
pub use snapshot::{SnapshotWitnessExt, WeightWitnessV1};
