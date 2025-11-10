#![forbid(unsafe_code)]

//! Production-ready Proof-of-Trust (PoT) validator node runtime.
//!
//! This module glues the low-level consensus primitives (`pot.rs`,
//! `snapshot.rs`, and the RANDAO beacon) into a high-level node abstraction
//! that can be used by a P2P service. The implementation focuses on three
//! goals:
//!
//! 1. Deterministic, verifiable leader election driven by PoT witnesses.
//! 2. Robust equivocation detection with immediate economic penalties.
//! 3. Safe epoch transitions (RANDAO finalization + weight snapshots).
//!
//! The node keeps all state in safe Rust structures and exposes a clear API
//! for higher layers (networking, storage, RPC). The `tt_node` binary builds
//! on top of this runtime.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use thiserror::Error;

use crate::pot::{
    detect_equivocation, finalize_epoch_and_slash, q_from_basis_points, slash_equivocation,
    verify_leader_with_witness, EpochSnapshot, NodeId, PotParams, Proposal, RandaoBeacon, Registry,
    TrustParams, TrustState, Q,
};
use crate::snapshot::SnapshotWitnessExt;
use crate::snapshot::WeightWitnessV1;

/// Identifier for an epoch/slot pair.
type SlotKey = (u64, u64);

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
        Self {
            who,
            stake,
            active: true,
            trust_override: None,
        }
    }
}

/// Result of processing a block proposal.
#[derive(Clone, Debug)]
pub struct SlotDecision {
    /// Slot number within the current epoch.
    pub slot: u64,
    /// Epoch number the slot belongs to.
    pub epoch: u64,
    /// Identity of the winning proposer.
    pub proposer: NodeId,
    /// Leader selection weight (higher = stronger priority).
    pub weight: u128,
    /// Block header commitment chosen by the proposer.
    pub header_hash: [u8; 32],
    /// Final trust after applying rewards/penalties for this proposal.
    pub trust_after: Q,
    /// True if the proposal replaced a previous winner for the slot.
    pub replaced_previous_winner: bool,
    /// True if the proposal triggered an equivocation slash.
    pub equivocation_slashed: bool,
}

/// Winning proposal for a slot.
#[derive(Clone, Debug)]
pub struct SlotWinner {
    /// Slot number associated with the winner.
    pub slot: u64,
    /// Epoch number for which the slot was elected.
    pub epoch: u64,
    /// Validator identity that produced the winning block.
    pub who: NodeId,
    /// Winning weight used to compare with other proposals.
    pub weight: u128,
    /// Block header hash committed by the leader.
    pub header_hash: [u8; 32],
}

/// Errors that can happen while handling consensus events.
#[derive(Debug, Error)]
pub enum NodeError {
    /// Proposal references a different epoch than the active snapshot.
    #[error("epoch mismatch (expected {expected}, got {received})")]
    EpochMismatch { expected: u64, received: u64 },
    /// Validator is not present in the current snapshot.
    #[error("validator not part of active snapshot")]
    UnknownValidator,
    /// Provided witness does not authenticate against the snapshot Merkle root.
    #[error("invalid weight witness")]
    InvalidWitness,
    /// Validator did not win the slot lottery for the provided slot.
    #[error("validator not eligible for slot")]
    NotEligible,
}

/// Production-ready Proof-of-Trust node state machine.
pub struct PotNode {
    config: PotNodeConfig,
    registry: Registry,
    trust: TrustState,
    beacon: RandaoBeacon,
    snapshot: EpochSnapshot,
    /// Proposals grouped by epoch+slot, then by validator.
    proposals: HashMap<SlotKey, HashMap<NodeId, Vec<Proposal>>>,
    /// Winning proposal per slot.
    winners: HashMap<SlotKey, SlotWinner>,
    /// Guard to avoid double-slashing for the same equivocation event.
    slashed_slots: HashSet<(u64, u64, NodeId)>,
}

impl PotNode {
    /// Create a new node from genesis validators and beacon seed.
    pub fn new(
        config: PotNodeConfig,
        genesis_validators: Vec<GenesisValidator>,
        genesis_beacon: [u8; 32],
    ) -> Self {
        let mut registry = Registry::default();
        let mut trust = TrustState::default();

        for v in genesis_validators {
            registry.insert(v.who, v.stake, v.active);
            let initial_trust = v.trust_override.unwrap_or(config.params.trust.init_q);
            trust.set(v.who, initial_trust);
        }

        let mut beacon = RandaoBeacon::new(config.params.slash_noreveal_bps, genesis_beacon);
        // Genesis epoch is 0.
        let snapshot = EpochSnapshot::build(
            0,
            &registry,
            &trust,
            &config.params.trust,
            config.params.min_bond,
        );
        // Seed for epoch 0 must match genesis beacon.
        beacon.prev_beacon = genesis_beacon;

        Self {
            config,
            registry,
            trust,
            beacon,
            snapshot,
            proposals: HashMap::new(),
            winners: HashMap::new(),
            slashed_slots: HashSet::new(),
        }
    }

    /// Access node configuration.
    pub fn config(&self) -> &PotNodeConfig {
        &self.config
    }

    /// Active registry view.
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Trust ledger.
    pub fn trust(&self) -> &TrustState {
        &self.trust
    }

    /// Current epoch snapshot (start-of-epoch weights).
    pub fn snapshot(&self) -> &EpochSnapshot {
        &self.snapshot
    }

    /// Current RANDAO beacon state.
    pub fn beacon(&self) -> &RandaoBeacon {
        &self.beacon
    }

    /// Winning proposals collected so far.
    pub fn winners(&self) -> &HashMap<SlotKey, SlotWinner> {
        &self.winners
    }

    /// Record a RANDAO commitment for the given epoch.
    pub fn record_randao_commit(&mut self, epoch: u64, who: NodeId, commitment: [u8; 32]) {
        self.beacon.commit(epoch, who, commitment);
    }

    /// Record a RANDAO reveal for the given epoch.
    pub fn record_randao_reveal(&mut self, epoch: u64, who: NodeId, reveal: [u8; 32]) -> bool {
        self.beacon.reveal(epoch, who, reveal)
    }

    /// Process a block proposal validated by a compact witness.
    pub fn process_proposal(
        &mut self,
        epoch: u64,
        slot: u64,
        witness: &WeightWitnessV1,
        header_hash: [u8; 32],
    ) -> Result<SlotDecision, NodeError> {
        if epoch != self.snapshot.epoch {
            return Err(NodeError::EpochMismatch {
                expected: self.snapshot.epoch,
                received: epoch,
            });
        }

        if self.snapshot.order.iter().all(|id| id != &witness.who) {
            return Err(NodeError::UnknownValidator);
        }

        if !self.snapshot.verify_witness(witness) {
            return Err(NodeError::InvalidWitness);
        }

        let weight = verify_leader_with_witness(
            &self.registry,
            &self.snapshot,
            &self.beacon,
            &mut self.trust,
            &self.config.params,
            epoch,
            slot,
            witness,
        )
        .ok_or(NodeError::NotEligible)?;

        let slot_key = (epoch, slot);
        let per_slot = self.proposals.entry(slot_key).or_default();
        let proposals = per_slot.entry(witness.who).or_default();
        proposals.push(Proposal {
            who: witness.who,
            slot,
            header_hash,
        });

        let slash_key = (epoch, slot, witness.who);
        let mut equivocation_slashed = false;
        if detect_equivocation(proposals) {
            if !self.slashed_slots.contains(&slash_key) {
                slash_equivocation(
                    &mut self.registry,
                    &mut self.trust,
                    &witness.who,
                    self.config.params.trust,
                    self.config.equivocation_penalty_bps,
                );
                self.slashed_slots.insert(slash_key);
            }
            equivocation_slashed = true;
            self.winners.remove(&slot_key);
        }

        let mut replaced_previous_winner = false;
        if !equivocation_slashed {
            let entry = self.winners.entry(slot_key).or_insert(SlotWinner {
                slot,
                epoch,
                who: witness.who,
                weight,
                header_hash,
            });

            if entry.who == witness.who && entry.header_hash == header_hash {
                // Duplicate proposal, keep existing winner.
            } else if weight > entry.weight {
                *entry = SlotWinner {
                    slot,
                    epoch,
                    who: witness.who,
                    weight,
                    header_hash,
                };
                replaced_previous_winner = true;
            }
        }

        let trust_after = self
            .trust
            .get(&witness.who, self.config.params.trust.init_q);

        Ok(SlotDecision {
            slot,
            epoch,
            proposer: witness.who,
            weight,
            header_hash,
            trust_after,
            replaced_previous_winner,
            equivocation_slashed,
        })
    }

    /// Finalize the active epoch, applying RANDAO penalties and building a new snapshot.
    pub fn finalize_epoch(&mut self) -> [u8; 32] {
        let finished_epoch = self.snapshot.epoch;
        let beacon = finalize_epoch_and_slash(
            &mut self.beacon,
            finished_epoch,
            &mut self.registry,
            &mut self.trust,
            self.config.params.trust,
        );
        // Prepare snapshot for next epoch.
        let next_epoch = finished_epoch + 1;
        self.snapshot = EpochSnapshot::build(
            next_epoch,
            &self.registry,
            &self.trust,
            &self.config.params.trust,
            self.config.params.min_bond,
        );
        self.proposals.clear();
        self.winners.clear();
        self.slashed_slots.clear();
        beacon
    }

    /// Helper for tests and tooling: returns a compact witness for a validator.
    pub fn witness_for(&self, who: &NodeId) -> Option<WeightWitnessV1> {
        self.snapshot.build_compact_witness(who)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::crypto_kmac_consensus::kmac256_hash;
    use crate::pot::ONE_Q;

    fn nid(n: u8) -> NodeId {
        let mut id = [0u8; 32];
        id[0] = n;
        id
    }

    fn default_config(node_id: NodeId) -> PotNodeConfig {
        PotNodeConfig {
            node_id,
            slot_duration: Duration::from_secs(1),
            epoch_length: 8,
            params: PotParams {
                trust: TrustParams {
                    alpha_q: q_from_basis_points(9900),
                    beta_q: q_from_basis_points(100),
                    init_q: q_from_basis_points(5000),
                },
                lambda_q: ONE_Q, // deterministic eligibility for tests
                min_bond: 1,
                slash_noreveal_bps: 0,
            },
            equivocation_penalty_bps: 5_000,
        }
    }

    #[test]
    fn node_initializes_snapshot() {
        let node_id = nid(0);
        let cfg = default_config(node_id);
        let validators = vec![
            GenesisValidator::active(nid(1), 1),
            GenesisValidator::active(nid(2), 2),
        ];
        let node = PotNode::new(cfg, validators, [7u8; 32]);
        assert_eq!(node.snapshot().epoch, 0);
        assert_eq!(node.snapshot().order.len(), 2);
        assert_eq!(node.registry().map.len(), 2);
    }

    #[test]
    fn process_valid_proposal_updates_winner() {
        let node_id = nid(0);
        let cfg = default_config(node_id);
        let validators = vec![GenesisValidator::active(nid(1), 1)];
        let mut node = PotNode::new(cfg, validators, [3u8; 32]);
        let who = nid(1);
        let witness = node.witness_for(&who).expect("witness");
        let header_hash = kmac256_hash(b"TEST.header", &[&0u64.to_le_bytes(), &who]);
        let decision = node
            .process_proposal(0, 0, &witness, header_hash)
            .expect("proposal accepted");
        assert!(!decision.equivocation_slashed);
        assert!(node.winners().contains_key(&(0, 0)));
    }

    #[test]
    fn equivocation_triggers_slash() {
        let node_id = nid(0);
        let cfg = default_config(node_id);
        let validators = vec![GenesisValidator::active(nid(1), 10)];
        let mut node = PotNode::new(cfg, validators, [9u8; 32]);
        let who = nid(1);
        let witness = node.witness_for(&who).expect("witness");
        let header_a = kmac256_hash(b"TEST.header", &[&0u64.to_le_bytes(), &who, b"A"]);
        let header_b = kmac256_hash(b"TEST.header", &[&0u64.to_le_bytes(), &who, b"B"]);

        let _ = node
            .process_proposal(0, 0, &witness, header_a)
            .expect("first proposal");
        let decision = node
            .process_proposal(0, 0, &witness, header_b)
            .expect("second proposal");
        assert!(decision.equivocation_slashed);
        let stake_after = node.registry().stake(&who);
        assert!(stake_after < 10);
    }
}
