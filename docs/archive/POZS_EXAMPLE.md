# PoZS Usage Examples

## Example 1: Basic ZK Witness Creation

\`\`\`rust
use tt_priv_cli::pozs::{ZkLeaderWitness, ZkProver, ZkScheme};
use tt_priv_cli::pot::q_from_basis_points;

// Create prover
let prover = ZkProver::new(ZkScheme::Groth16BN254)?;

// Validator won sortition for (epoch=42, slot=7)
let beacon_value = beacon.value(42, 7);

// Generate ZK proof of eligibility
let proof = prover.prove_eligibility(
    &beacon_value,
    7,                              // slot
    &my_validator_id,
    q_from_basis_points(5000),      // 50% stake
    q_from_basis_points(8500),      // 85% trust
    q_from_basis_points(1200),      // 12% threshold
)?;

// Create witness for block
let witness = ZkLeaderWitness {
    who: my_validator_id,
    slot: 7,
    epoch: 42,
    weights_root: snapshot.weights_root,
    merkle_proof: None,             // Skip classical proof
    zk_proof: Some(proof),          // Use ZK instead
    stake_q: q_from_basis_points(5000),
    trust_q: q_from_basis_points(8500),
};
\`\`\`

## Example 2: Hybrid Verification

\`\`\`rust
use tt_priv_cli::pozs::{verify_leader_zk, ZkVerifier, ZkScheme};

// Initialize verifier (once at startup)
let verifier = ZkVerifier::new(ZkScheme::Groth16BN254)?;

// Verify incoming block
let weight = verify_leader_zk(
    &registry,
    &epoch_snapshot,
    &beacon,
    &mut trust_state,
    &params,
    &block.witness,
    Some(&verifier),  // Enable ZK verification
)?;

println!("Block accepted! Weight: {}", weight);
\`\`\`

## Example 3: Backward-Compatible Verification

\`\`\`rust
// Node that supports BOTH Merkle and ZK

match &witness.zk_proof {
    Some(proof) => {
        // Fast path: ZK verification
        info!("Verifying ZK proof ({} bytes)", proof.proof_bytes.len());
        verify_leader_zk(&registry, &snapshot, &beacon, 
                        &mut trust, &params, &witness, 
                        Some(&verifier))?
    }
    None => {
        // Fallback: classical verification
        info!("Using classical Merkle verification");
        verify_leader_with_witness(&registry, &snapshot, &beacon,
                                   &mut trust, &params, 
                                   witness.epoch, witness.slot,
                                   &witness.to_merkle_witness())?
    }
}
\`\`\`

## Example 4: Recursive Aggregation (Future)

\`\`\`rust
use tt_priv_cli::pozs::AggregatedProof;

// Aggregate 100 blocks into 1 proof
let mut agg = AggregatedProof::new(epoch, start_slot);

for witness in block_witnesses {
    agg.fold(&witness)?;
}

// Sync node verifies 100 blocks with 1 pairing check
assert!(agg.verify(&verifier)?);
println!("Verified {} blocks in {}ms", agg.block_count, elapsed);
\`\`\`

## Example 5: Integration with PotNode

\`\`\`rust
use tt_priv_cli::{PotNode, PotNodeConfig};
use tt_priv_cli::pozs::{ZkProver, ZkScheme};

pub struct ZkPotNode {
    node: PotNode,
    prover: Option<ZkProver>,
    verifier: Option<ZkVerifier>,
}

impl ZkPotNode {
    pub fn new_with_zk(
        config: PotNodeConfig,
        validators: Vec<GenesisValidator>,
        genesis_beacon: [u8; 32],
        enable_zk: bool,
    ) -> Result<Self> {
        let node = PotNode::new(config, validators, genesis_beacon);
        
        let (prover, verifier) = if enable_zk {
            let p = ZkProver::new(ZkScheme::Groth16BN254)?;
            let v = ZkVerifier::new(ZkScheme::Groth16BN254)?;
            (Some(p), Some(v))
        } else {
            (None, None)
        };
        
        Ok(Self { node, prover, verifier })
    }
    
    pub fn propose_block_zk(&mut self, slot: u64) -> Result<ZkLeaderWitness> {
        let prover = self.prover.as_ref()
            .ok_or_else(|| anyhow!("ZK not enabled"))?;
        
        // Generate witness with ZK proof
        let witness = self.node.witness_for(&my_id)?;
        let proof = prover.prove_eligibility(
            &self.node.beacon().value(epoch, slot),
            slot,
            &my_id,
            witness.stake_q,
            witness.trust_q,
            threshold,
        )?;
        
        Ok(ZkLeaderWitness {
            who: my_id,
            slot,
            epoch,
            weights_root: self.node.snapshot().weights_root,
            merkle_proof: None,
            zk_proof: Some(proof),
            stake_q: witness.stake_q,
            trust_q: witness.trust_q,
        })
    }
}
\`\`\`

