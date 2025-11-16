//! TRUE_TRUST Proof-of-Trust consensus module
//! 
//! This module implements a Proof-of-Trust consensus mechanism with:
//! - Trust decay/reward system
//! - RANDAO commit-reveal beacon
//! - Merkle tree-based weight snapshots
//! - Sortition-based leader selection
//! - Equivocation detection and slashing
//! 
//! Production blockchain modules:
//! - bp: Bulletproofs verification for 64-bit range proofs
//! - chain: Chain storage with orphan handling and cumulative weight
//! - core: Core blockchain primitives (Hash32, Block, timestamp utilities)
//! - state: Public blockchain state (balances, trust, keyset, nonces)
//! - state_priv: Private blockchain state (notes_root, nullifiers, frontier)
//! - consensus: Trust-based consensus primitives
//! - zk: RISC0 zkVM integration for private transactions
//! - node: Production blockchain node integrating all components

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

// Production blockchain modules
pub mod bp;
pub mod chain;
pub mod core;
pub mod state;
pub mod state_priv;
pub mod consensus;
pub mod zk;
pub mod node;  // Production node v2 with split BP verifiers
pub mod tx;
pub mod cpu_proof; // CPU-only micro PoW and proof metrics
pub mod cpu_mining; // Hybrid PoT+PoS+MicroPoW optimized for old CPUs
pub mod falcon_sigs; // Falcon-512 Post-Quantum Signatures
pub mod kyber_kem; // Kyber-768 Post-Quantum KEM
pub mod pozs_lite;
pub mod zk_trust; // Privacy-preserving trust proofs
pub mod golden_trio; // Golden Trio consensus (PoT + RandomX + PoS)
pub mod randomx_full; // Full RandomX (Pure Rust implementation - fallback)

// RandomX FFI - tylko jeśli RANDOMX_FFI=1 (wymaga biblioteki C)
#[cfg(all(feature = "randomx-ffi", target_pointer_width = "64"))]
pub mod pow_randomx_monero; // OFFICIAL RandomX FFI (Monero C library) - PRODUCTION!

pub mod rtt_trust; // RTT (Recursive Trust Tree) - f64 version
pub mod rtt_trust_pro; // RTT PRO (Q32.32 deterministic) - PRODUCTION!
pub mod consensus_pro; // Unified facade: RTT PRO + RandomX + Golden Trio + STARK
// STARK implementations
pub mod stark_full; // BabyBear (31-bit, educational/testing only)
pub mod stark_goldilocks; // Goldilocks (64-bit, production target) - RECOMMENDED!
pub mod stark_security; // Security analysis & parameter validation
// pub mod stark_winterfell_simple; // Winterfell placeholder (API mismatch 0.9 vs 0.13+)

pub mod tx_stark; // Post-Quantum transactions (STARK range proofs, replaces Bulletproofs)
pub mod p2p_secure; // PQ-secure P2P transport (Falcon512 + Kyber768 + XChaCha20-Poly1305)
pub mod node_v2_p2p; // Blockchain node with PQ P2P networking

// Post-Quantum module (Winterfell STARK + PQ TX)
pub mod pq;

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
