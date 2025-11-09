//! Quantum Falcon Wallet - Advanced Post-Quantum Cryptography
//! 
//! KMAC256 + Falcon512 hybrid keysearch implementation
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod crypto;
pub mod crypto_kmac;
pub mod keysearch;
pub mod consensus;
pub mod snapshot;
pub mod hybrid_commit;  // ✅ NEW: PQC-aware hybrid commitments (Idea 4)
pub mod bp;  // ✅ NEW: Bulletproofs 64-bit (classical, for ZK guests)
pub mod falcon_sigs;  // ✅ NEW: Falcon-512 signature operations
pub mod pqc_verify;  // ✅ NEW: Host-side PQC signature verification

#[cfg(feature = "tt-full")]
pub mod tt_quantum_wallet;  // ✅ NOWY: Quantum wallet integration

#[cfg(feature = "tt-full")]
pub mod tt_cli;  // ✅ NOWY: Full CLI

#[cfg(feature = "tt-full")]
pub mod tt_priv_cli;  // ✅ NOWY: Complete standalone CLI v5

// Re-export main types
pub use crypto::{
    QuantumKeySearchCtx,
    QuantumSafeHint,
    QuantumFoundNote,
    FalconKeyManager,
    FalconError,
    kmac256_derive_key,
};

pub use keysearch::{
    HintPayloadV1,
    DecodedHint,
    KeySearchCtx,
    AadMode,
};

pub use consensus::{
    Q, ONE_Q, NodeId, StakeQ,
    TrustParams, TrustState,
    Registry, RegEntry,
    EpochSnapshot, MerkleProof,
    RandaoBeacon, RandaoEpoch,
    LeaderWitness, PotParams,
    q_from_ratio, q_from_basis_points,
    verify_leader_and_update_trust,
    verify_leader_with_witness,
    detect_equivocation,
    slash_equivocation,
    finalize_epoch_and_slash,
};

pub use snapshot::{
    WeightWitnessV1,
    SnapshotWitnessExt,
};

#[cfg(feature = "tt-full")]
pub use tt_quantum_wallet::{
    Keyset as QuantumKeyset,
    WalletFile as QuantumWalletFile,
    WalletSecretPayloadV3,
    create_wallet_v3,
    bech32_addr,
    bech32_addr_quantum,
};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Check quantum support
pub fn has_quantum_support() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_version() {
        assert!(!VERSION.is_empty());
        println!("Library version: {}", VERSION);
    }

    #[test]
    fn test_quantum_available() {
        assert!(has_quantum_support());
    }
}
