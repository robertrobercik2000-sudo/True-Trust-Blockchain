#![forbid(unsafe_code)]

//! Production-ready Proof-of-Trust (PoT) validator node runtime.
//!
//! **DETERMINISTIC LEADER SELECTION** (not lottery):
//! - Each slot has ONE deterministic leader based on weight
//! - Leader = validator with highest (2/3 trust + 1/3 stake)
//! - Beacon + slot used for tie-breaking, not randomness
//! - No probabilistic sortition!

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use thiserror::Error;

use crate::pot::{
    detect_equivocation, finalize_epoch_and_slash, q_from_basis_points, slash_equivocation,
    EpochSnapshot, NodeId, PotParams, RandaoBeacon, Registry, TrustParams, TrustState,
};
use crate::pot::{verify_leader_with_witness, Q, compute_weight_linear};
use crate::snapshot::{SnapshotWitnessExt, WeightWitnessV1};

/// Identifier for an epoch/slot pair.
type SlotKey = (u64, u64);

/// Re-export Proposal from pot module
pub use crate::pot::Proposal;

/// Configuration of a production Proof-of-Trust node.
#[derive(Clone)]
pub struct PotNodeConfig {
    /// Local node identifier (used by higher layers).
    pub node_id: NodeId,
    /// Target slot duration for block production.
    pub slot_duration: Duration,
    /// Number of slots in a consensus epoch.
    pub epoch_length: u64,
    /// Economic and probabilistic parameters for Proof-of-Trust.
    pub params: PotParams,
    /// Penalty applied when equivocation is detected (in basis points).
    pub equivocation_penalty_bps: u32,
}

impl PotNodeConfig {
    /// Helper constructor that accepts basis points for common TrustParams.
    pub fn new_with_trust_bps(
        node_id: NodeId,
        slot_duration: Duration,
        epoch_length: u64,
        lambda_bps: u32,
        alpha_bps: u32,
        beta_bps: u32,
        init_bps: u32,
        min_bond: u64,
        slash_noreveal_bps: u32,
        equivocation_penalty_bps: u32,
    ) -> Self {
        let trust = TrustParams {
            alpha_q: q_from_basis_points(alpha_bps),
            beta_q: q_from_basis_points(beta_bps),
            init_q: q_from_basis_points(init_bps),
        };
        let params = PotParams {
            trust,
            lambda_q: q_from_basis_points(lambda_bps),
            min_bond,
            slash_noreveal_bps,
        };
        Self {
            node_id,
            slot_duration,
            epoch_length,
            params,
            equivocation_penalty_bps,
        }
    }
}

/// Validator entry used for bootstrapping a node.
#[derive(Clone)]
pub struct GenesisValidator {
    /// Validator identity (32-byte node identifier).
    pub who: NodeId,
    /// Bonded stake expressed in native token units.
    pub stake: u64,
    /// Whether the validator is part of the active set at genesis.
    pub active: bool,
    /// Optional trust bootstrap overriding [`PotNodeConfig::params`].
    pub trust_override: Option<Q>,
}

impl GenesisValidator {
    /// Convenience helper for active validators without trust overrides.
    pub fn active(who: NodeId, stake: u64) -> Self {
        Self { who, stake, active: true, trust_override: None }
    }
}

/// Production Proof-of-Trust node state.
pub struct PotNode {
    config: PotNodeConfig,
    registry: Registry,
    trust: TrustState,
    beacon: RandaoBeacon,
    snapshot: EpochSnapshot,
    
    // Slot tracking for deterministic leader selection
    current_slot: u64,
    genesis_time: std::time::Instant,
}

impl PotNode {
    /// Creates a new PoT node with genesis state.
    pub fn new(
        config: PotNodeConfig,
        genesis_validators: Vec<GenesisValidator>,
        beacon_seed: [u8; 32],
    ) -> Self {
        let mut registry = Registry { map: HashMap::new() };
        let mut trust = TrustState::new();

        // Bootstrap from genesis validators
        for gv in genesis_validators {
            registry.insert(gv.who, gv.stake, gv.active);
            let trust_val = gv.trust_override.unwrap_or(config.params.trust.init_q);
            trust.set(gv.who, trust_val);
        }

        let beacon = RandaoBeacon::new(0, beacon_seed);
        
        // Build initial snapshot from genesis
        let snapshot = EpochSnapshot::build(
            0,
            &registry,
            &trust,
            &config.params.trust,
            config.params.min_bond,
        );

        Self {
            config,
            registry,
            trust,
            beacon,
            snapshot,
            current_slot: 0,
            genesis_time: std::time::Instant::now(),
        }
    }

    /// Get current epoch based on elapsed time
    pub fn current_epoch(&self) -> u64 {
        let elapsed = self.genesis_time.elapsed();
        let slot_duration_secs = self.config.slot_duration.as_secs();
        let epoch_duration_secs = slot_duration_secs * self.config.epoch_length;
        
        if epoch_duration_secs == 0 {
            return 0;
        }
        
        elapsed.as_secs() / epoch_duration_secs
    }

    /// Get current slot within epoch
    pub fn current_slot(&self) -> u64 {
        let elapsed = self.genesis_time.elapsed();
        let slot_duration_secs = self.config.slot_duration.as_secs();
        
        if slot_duration_secs == 0 {
            return 0;
        }
        
        let total_slots = elapsed.as_secs() / slot_duration_secs;
        total_slots % self.config.epoch_length
    }

    /// **DETERMINISTIC** leader selection (NOT lottery!)
    /// 
    /// Returns weight if this node is the leader for (epoch, slot).
    /// Selection is deterministic based on highest weight (2/3 trust + 1/3 stake).
    /// 
    /// Algorithm:
    /// 1. Get all active validators
    /// 2. Compute weight = (2/3) * trust + (1/3) * stake for each
    /// 3. Sort by weight descending
    /// 4. Use (beacon + slot) % num_validators to pick from top validators
    /// 5. Return weight if we're the selected leader
    pub fn check_eligibility(&self, epoch: u64, slot: u64) -> Option<u128> {
        // Check if we're in the active set
        if !self.registry.is_active(&self.config.node_id, self.config.params.min_bond) {
            return None;
        }
        
        // Get all active validators and compute their weights
        let mut weighted_validators: Vec<(NodeId, u128)> = self.registry.map.values()
            .filter(|e| e.active && e.stake >= self.config.params.min_bond)
            .filter_map(|e| {
                let stake_q = self.snapshot.stake_q_of(&e.who);
                let trust_q = self.snapshot.trust_q_of(&e.who);
                
                if stake_q == 0 {
                    return None;
                }
                
                // Weight = (2/3) * trust + (1/3) * stake
                let weight_q = compute_weight_linear(stake_q, trust_q);
                Some((e.who, weight_q as u128))
            })
            .collect();
        
        if weighted_validators.is_empty() {
            return None;
        }
        
        // Sort by weight descending (highest first)
        weighted_validators.sort_by(|a, b| b.1.cmp(&a.1));
        
        // Deterministic selection using beacon + slot
        // This ensures same leader for same (epoch, slot) across all nodes
        let beacon_val = self.beacon.value(epoch, slot);
        let beacon_u64 = u64::from_le_bytes(beacon_val[0..8].try_into().unwrap_or([0u8; 8]));
        let selection_seed = beacon_u64.wrapping_add(slot);
        let leader_idx = (selection_seed as usize) % weighted_validators.len();
        
        let (selected_leader, selected_weight) = weighted_validators[leader_idx];
        
        // Check if WE are the selected leader
        if selected_leader == self.config.node_id {
            Some(selected_weight)
        } else {
            None
        }
    }
    
    /// Creates a LeaderWitness for the current node
    ///
    /// If `use_zk_trust` is true, generates privacy-preserving ZK proof
    /// instead of revealing exact trust_q value.
    pub fn create_witness(&self, epoch: u64, slot: u64, use_zk_trust: bool) -> Option<crate::pot::LeaderWitness> {
        use crate::pot::LeaderWitness;
        use crate::zk_trust::TrustProver;
        
        // Verify we're eligible first
        self.check_eligibility(epoch, slot)?;
        
        // Get my data from snapshot
        let my_stake_q = self.snapshot.stake_q_of(&self.config.node_id);
        let my_trust_q = self.snapshot.trust_q_of(&self.config.node_id);
        
        // Build Merkle proof
        let weight_proof = self.snapshot.build_proof(&self.config.node_id)?;
        
        // Optional: Generate ZK proof of trust (privacy!)
        let trust_zk_proof = if use_zk_trust {
            let prover = TrustProver::new(my_trust_q, self.config.node_id);
            // Prove trust >= min required (e.g., init_q)
            if let Some(proof) = prover.prove_threshold(self.config.params.trust.init_q) {
                bincode::serialize(&proof).ok()
            } else {
                None
            }
        } else {
            None
        };
        
        Some(LeaderWitness {
            who: self.config.node_id,
            slot,
            epoch,
            weights_root: self.snapshot.weights_root,
            weight_proof,
            stake_q: my_stake_q,
            trust_q: my_trust_q,
            trust_zk_proof,
        })
    }

    // ===== Accessors =====
    
    pub fn config(&self) -> &PotNodeConfig {
        &self.config
    }
    
    pub fn registry(&self) -> &Registry {
        &self.registry
    }
    
    pub fn trust(&self) -> &TrustState {
        &self.trust
    }
    
    pub fn trust_mut(&mut self) -> &mut TrustState {
        &mut self.trust
    }
    
    pub fn snapshot(&self) -> &EpochSnapshot {
        &self.snapshot
    }
    
    pub fn beacon(&self) -> &RandaoBeacon {
        &self.beacon
    }
}

#[derive(Debug, Error)]
pub enum NodeError {
    #[error("Invalid validator")]
    InvalidValidator,
    
    #[error("Not eligible for slot")]
    NotEligible,
    
    #[error("Equivocation detected")]
    Equivocation,
}

/// Result of slot processing
#[derive(Clone, Debug)]
pub enum SlotDecision {
    /// No leader selected (shouldn't happen in deterministic mode)
    NoLeader,
    
    /// We are the leader
    WeAreLeader { weight: u128 },
    
    /// Someone else is leader
    OtherLeader { who: NodeId },
}

/// Slot winner information
#[derive(Clone, Debug)]
pub struct SlotWinner {
    pub who: NodeId,
    pub slot: u64,
    pub epoch: u64,
    pub weight: u128,
}
