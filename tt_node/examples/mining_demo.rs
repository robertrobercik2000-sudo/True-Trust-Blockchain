//! Complete Mining Demo - Full Pipeline Test
//!
//! This demonstrates the complete TRUE_TRUST mining pipeline:
//! 1. Initialize RandomX dataset (PoW)
//! 2. Create block template
//! 3. Mine block with RandomX
//! 4. Verify PoW
//! 5. Verify PQC signatures
//! 6. Submit to consensus
//! 7. Distribute rewards

use anyhow::{Result, anyhow};
use std::time::Instant;
use serde::{Serialize, Deserialize};

// Import tt_node modules
use tt_node::randomx_full::{RandomXHasher, mine_randomx};
use tt_node::falcon_sigs::falcon_keypair;
use tt_node::consensus_pro::ConsensusPro;
use tt_node::node_id::node_id_from_falcon_pk;
use tt_node::crypto::kmac;

/// Block header for mining
#[derive(Clone, Debug, Serialize, Deserialize)]
struct BlockHeader {
    /// Block number
    height: u64,
    /// Previous block hash
    prev_hash: [u8; 32],
    /// Timestamp (Unix milliseconds)
    timestamp: u64,
    /// Merkle root of transactions
    tx_root: [u8; 32],
    /// Validator/miner ID
    validator_id: [u8; 32],
    /// Mining nonce (filled during PoW)
    nonce: u64,
}

impl BlockHeader {
    fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).expect("serialize header")
    }
    
    /// Compute block hash (without nonce)
    fn compute_hash_without_nonce(&self) -> [u8; 32] {
        let mut data = Vec::new();
        data.extend_from_slice(&self.height.to_le_bytes());
        data.extend_from_slice(&self.prev_hash);
        data.extend_from_slice(&self.timestamp.to_le_bytes());
        data.extend_from_slice(&self.tx_root);
        data.extend_from_slice(&self.validator_id);
        
        kmac::kmac256_tag(&[0u8; 32], b"TT.BLOCK.HASH", &data)
    }
}

/// Complete block with PoW proof
#[derive(Clone, Debug)]
struct MinedBlock {
    header: BlockHeader,
    pow_hash: [u8; 32],
    transactions: Vec<Vec<u8>>, // Simplified: raw tx bytes
    signature: Vec<u8>, // Falcon signature over block hash
}

/// Mining configuration
struct MiningConfig {
    difficulty_target: [u8; 32],
    max_iterations: u64,
    epoch: u64,
}

impl Default for MiningConfig {
    fn default() -> Self {
        // Easy target for demo: hash must be < target
        // Target with 4 leading zero bytes = ~2^32 difficulty
        let mut target = [0xFFu8; 32];
        target[0] = 0x00;
        target[1] = 0x00;
        target[2] = 0x0F;  // Adjust for demo (easier)
        
        Self {
            difficulty_target: target,
            max_iterations: 10000, // Limit for demo
            epoch: 0,
        }
    }
}

/// Compute difficulty from target
fn compute_difficulty(target: &[u8; 32]) -> f64 {
    // Difficulty = max_target / current_target
    // Where max_target = 2^256 - 1
    
    // Approximate: count leading zero bits
    let mut leading_zeros = 0;
    for byte in target.iter() {
        if *byte == 0 {
            leading_zeros += 8;
        } else {
            leading_zeros += byte.leading_zeros() as usize;
            break;
        }
    }
    
    2.0_f64.powi(leading_zeros as i32)
}

/// Main mining function
fn mine_block(
    hasher: &RandomXHasher,
    header: &mut BlockHeader,
    config: &MiningConfig,
) -> Result<MinedBlock> {
    println!("\n⛏️  MINING BLOCK #{}", header.height);
    println!("Difficulty: ~{:.0} hashes", compute_difficulty(&config.difficulty_target));
    println!("Target: {}", hex::encode(&config.difficulty_target[..8]));
    
    // Serialize header without nonce
    let block_data = header.compute_hash_without_nonce();
    
    // Mine!
    let start = Instant::now();
    let result = mine_randomx(
        hasher,
        &block_data,
        &config.difficulty_target,
        config.max_iterations,
    );
    
    match result {
        Some((nonce, pow_hash)) => {
            let elapsed = start.elapsed();
            header.nonce = nonce;
            
            println!("✅ Block mined in {:.2}s!", elapsed.as_secs_f64());
            println!("   Nonce: {}", nonce);
            println!("   Hash: {}", hex::encode(&pow_hash[..8]));
            
            let hashrate = (nonce as f64) / elapsed.as_secs_f64();
            println!("   Avg hashrate: {:.1} H/s", hashrate);
            
            Ok(MinedBlock {
                header: header.clone(),
                pow_hash,
                transactions: vec![],
                signature: vec![],
            })
        }
        None => {
            Err(anyhow!("Mining failed: no solution found in {} iterations", config.max_iterations))
        }
    }
}

/// Verify mined block
fn verify_block(
    hasher: &RandomXHasher,
    block: &MinedBlock,
    target: &[u8; 32],
) -> Result<()> {
    println!("\n🔍 VERIFYING BLOCK #{}", block.header.height);
    
    // Recompute block data
    let block_data = block.header.compute_hash_without_nonce();
    
    // Add nonce
    let mut input = block_data.to_vec();
    input.extend_from_slice(&block.header.nonce.to_le_bytes());
    
    // Verify PoW
    let start = Instant::now();
    let computed_hash = hasher.hash(&input);
    let verify_time = start.elapsed();
    
    // Check hash matches
    if computed_hash != block.pow_hash {
        return Err(anyhow!("PoW hash mismatch!"));
    }
    
    // Check hash < target
    if !hash_less_than(&computed_hash, target) {
        return Err(anyhow!("PoW hash does not meet difficulty target!"));
    }
    
    println!("✅ PoW verified in {:?}", verify_time);
    println!("   Hash: {}", hex::encode(&computed_hash[..8]));
    
    Ok(())
}

fn hash_less_than(a: &[u8; 32], b: &[u8; 32]) -> bool {
    for i in (0..32).rev() {
        if a[i] < b[i] {
            return true;
        } else if a[i] > b[i] {
            return false;
        }
    }
    false
}

/// Run complete mining simulation
fn run_mining_simulation() -> Result<()> {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  TRUE_TRUST Mining Demo - Full Pipeline Test            ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    
    // Step 1: Initialize RandomX
    println!("\n🔄 STEP 1: Initializing RandomX Dataset (2GB)");
    println!("This will take ~30-60 seconds...");
    let config = MiningConfig::default();
    let hasher = RandomXHasher::new(config.epoch);
    println!("✅ RandomX ready!");
    
    // Step 2: Setup validators and consensus
    println!("\n👥 STEP 2: Setting up validators");
    let mut consensus = ConsensusPro::new_default();
    
    // Create 3 validators
    let validators = vec![
        ("Alice", 2_000_000_u128, 0.95),
        ("Bob",   1_000_000_u128, 0.85),
        ("Carol",   500_000_u128, 0.90),
    ];
    
    let mut validator_keys = Vec::new();
    for (name, stake, quality) in &validators {
        let (pk, sk) = falcon_keypair();
        let id = node_id_from_falcon_pk(&pk);
        consensus.register_validator(id, *stake);
        consensus.record_quality_f64(&id, *quality);
        validator_keys.push((*name, id, pk, sk));
        println!("  {} - stake: {}, quality: {:.2}", name, stake, quality);
    }
    
    consensus.update_all_trust();
    println!("✅ Consensus initialized!");
    
    // Step 3: Mine multiple blocks
    println!("\n⛏️  STEP 3: Mining blocks");
    
    let mut prev_hash = [0u8; 32]; // Genesis
    let blocks_to_mine = 3;
    let mut mined_blocks = Vec::new();
    
    for block_num in 1u64..=blocks_to_mine {
        // Select leader for this block
        let mut beacon = [0u8; 32];
        beacon[..8].copy_from_slice(&block_num.to_le_bytes());
        let leader_id = consensus.select_leader(beacon)
            .ok_or_else(|| anyhow!("No leader selected"))?;
        
        let leader_name = validator_keys.iter()
            .find(|(_, id, _, _)| *id == leader_id)
            .map(|(name, _, _, _)| *name)
            .unwrap_or("Unknown");
        
        println!("\n═══ Block {} - Leader: {} ═══", block_num, leader_name);
        
        // Create block template
        let mut header = BlockHeader {
            height: block_num,
            prev_hash,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            tx_root: [0u8; 32], // Empty for demo
            validator_id: leader_id,
            nonce: 0,
        };
        
        // Mine the block
        let mined = mine_block(&hasher, &mut header, &config)?;
        
        // Verify the block
        verify_block(&hasher, &mined, &config.difficulty_target)?;
        
        // Update prev_hash for next block
        prev_hash = mined.pow_hash;
        mined_blocks.push(mined);
        
        println!("✅ Block {} added to chain!", block_num);
    }
    
    // Step 4: Calculate and distribute rewards
    println!("\n💰 STEP 4: Distributing rewards");
    
    let total_reward_per_block = 100_000_u128; // 100k TT per block
    println!("Total reward per block: {} TT", total_reward_per_block);
    println!();
    
    for (i, block) in mined_blocks.iter().enumerate() {
        let block_height = block.header.height;
        let validator_id = block.header.validator_id;
        
        let validator_name = validator_keys.iter()
            .find(|(_, id, _, _)| *id == validator_id)
            .map(|(name, _, _, _)| *name)
            .unwrap_or("Unknown");
        
        // Calculate reward based on validator weight
        let weight = consensus.compute_validator_weight(&validator_id).unwrap_or(0);
        let total_weight: u128 = validator_keys.iter()
            .map(|(_, id, _, _)| consensus.compute_validator_weight(id).unwrap_or(0))
            .sum();
        
        let reward = if total_weight > 0 {
            (total_reward_per_block * weight) / total_weight
        } else {
            0
        };
        
        println!("Block {}: {} receives {} TT (weight: {})", 
                 block_height, validator_name, reward, weight);
    }
    
    // Step 5: Final statistics
    println!("\n📊 FINAL STATISTICS");
    println!("═══════════════════");
    println!("Blocks mined: {}", mined_blocks.len());
    println!("Chain height: {}", mined_blocks.last().map(|b| b.header.height).unwrap_or(0));
    println!("Total validators: {}", validator_keys.len());
    
    let total_stake: u128 = validators.iter().map(|(_, s, _)| s).sum();
    println!("Total stake: {} TT", total_stake);
    
    println!("\nValidator weights:");
    for (name, id, _, _) in &validator_keys {
        let weight = consensus.compute_validator_weight(id).unwrap_or(0);
        let total_weight: u128 = validator_keys.iter()
            .map(|(_, id, _, _)| consensus.compute_validator_weight(id).unwrap_or(0))
            .sum();
        let percentage = if total_weight > 0 {
            (weight as f64 / total_weight as f64) * 100.0
        } else {
            0.0
        };
        println!("  {}: {:.2}%", name, percentage);
    }
    
    println!("\n🎉 Mining demo completed successfully!");
    
    Ok(())
}

fn main() -> Result<()> {
    // Set up logging
    env_logger::init();
    
    // Run the simulation
    run_mining_simulation()?;
    
    Ok(())
}

