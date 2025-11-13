//! TRUE_TRUST Proof-of-Trust consensus module
//! 
//! This module implements a Proof-of-Trust consensus mechanism with:
//! - Trust decay/reward system
//! - RANDAO commit-reveal beacon
//! - Merkle tree-based weight snapshots
//! - Sortition-based leader selection
//! - Equivocation detection and slashing

pub mod crypto_kmac_consensus;
pub mod pot;
pub mod pot_node;
pub mod pozs;
pub mod snapshot;

// Groth16 circuit implementation (requires zk-proofs feature)
#[cfg(feature = "zk-proofs")]
pub mod pozs_groth16;

// Keccak/KMAC256 gadgets for zkSNARK circuits (requires zk-proofs feature)
#[cfg(feature = "zk-proofs")]
pub mod pozs_keccak;

// Re-export main types for convenience
pub use pot::{
    EpochSnapshot, LeaderWitness, MerkleProof, NodeId, PotParams, Q, RandaoBeacon,
    Registry, TrustParams, TrustState, ONE_Q, q_from_basis_points, q_from_ratio,
    q_from_ratio128, verify_leader_and_update_trust, verify_leader_with_witness,
    detect_equivocation, slash_equivocation, finalize_epoch_and_slash,
};
pub use snapshot::{SnapshotWitnessExt, WeightWitnessV1};
pub use pot_node::{
    GenesisValidator, NodeError, PotNode, PotNodeConfig, Proposal, SlotDecision, SlotWinner,
};
pub use pozs::{
    AggregatedProof, ZkError, ZkLeaderWitness, ZkProof, ZkProver, ZkScheme, ZkVerifier,
    verify_leader_zk,
};

// Re-export Groth16 implementation when feature is enabled
#[cfg(feature = "zk-proofs")]
pub use pozs_groth16::{
    deserialize_proof, deserialize_vk, prove_eligibility, serialize_proof, serialize_vk,
    setup_keys, verify_eligibility, EligibilityCircuit, EligibilityPublicInputs,
    EligibilityWitness,
};
