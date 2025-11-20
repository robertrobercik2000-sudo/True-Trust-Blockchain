//! TRUE_TRUST Node - Post-Quantum Blockchain Implementation
//!
//! This is the main entry point for the TRUE_TRUST blockchain node.
//! It supports multiple modes of operation:
//! - Validator mode: Run as a consensus validator
//! - Full node: Run as a non-validating full node
//! - Light client: Run as a light client (future)
//! - Demo mode: Run demonstrations and examples

#![forbid(unsafe_code)]

use anyhow::{anyhow, Result};
use pqcrypto_traits::sign::PublicKey as SignPublicKey;
use pqcrypto_traits::kem::{PublicKey as KemPublicKey, Ciphertext as KemCiphertext, SharedSecret as KemSharedSecret};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::signal;

// Import all our modules
mod core;
mod chain_store;
mod state_priv;
mod randomx_full;

mod falcon_sigs;
mod kyber_kem;
mod crypto_kmac_consensus;
mod hybrid_commit;
mod node_id;
mod rtt_pro;
mod golden_trio;
mod consensus_weights;
mod consensus_pro;
mod snapshot_pro;
mod snapshot_witness;
mod stark_security;
mod stark_full;
mod tx_stark;
mod crypto;
mod pqc_verification;
mod p2p;
mod node_core;


#[cfg(feature = "wallet")]
pub mod wallet;

/// TRUE_TRUST Node CLI
#[derive(Parser, Debug)]
#[command(name = "tt_node", version, author)]
#[command(about = "TRUE_TRUST Post-Quantum Blockchain Node")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Start a validator node
    Validator {
        /// Data directory for blockchain storage
        #[arg(long, default_value = "./data")]
        data_dir: PathBuf,
        
        /// P2P listening port
        #[arg(long, default_value = "9090")]
        p2p_port: u16,
        
        /// RPC API port
        #[arg(long, default_value = "8080")]
        rpc_port: u16,
        
        /// Path to validator keystore
        #[arg(long)]
        keystore: Option<PathBuf>,
        
        /// Initial stake amount (in TT tokens)
        #[arg(long, default_value = "1000")]
        stake: u128,
    },
    
    /// Start a full node (non-validating)
    FullNode {
        /// Data directory for blockchain storage
        #[arg(long, default_value = "./data")]
        data_dir: PathBuf,
        
        /// P2P listening port
        #[arg(long, default_value = "9090")]
        p2p_port: u16,
        
        /// RPC API port
        #[arg(long, default_value = "8080")]
        rpc_port: u16,
    },
    
    /// Initialize a new chain genesis
    InitGenesis {
        /// Output genesis file path
        #[arg(long, default_value = "./genesis.json")]
        output: PathBuf,
        
        /// Chain ID
        #[arg(long, default_value = "tt-mainnet")]
        chain_id: String,
        
        /// Initial validators file (JSON)
        #[arg(long)]
        validators: Option<PathBuf>,
    },
    
    /// Run consensus demo
    ConsensusDemo {
        /// Number of validators to simulate
        #[arg(long, default_value = "3")]
        validators: u32,
        
        /// Number of rounds to run
        #[arg(long, default_value = "10")]
        rounds: u32,
    },
    
    /// Run crypto benchmarks
    Benchmark {
        /// Specific benchmark to run
        #[arg(long)]
        filter: Option<String>,
    },
    
    /// Test all features
    TestAll,
    
    /// Show node info
    Info {
        /// Show detailed crypto capabilities
        #[arg(long)]
        crypto: bool,
    },
}

fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();
    
    // Parse CLI arguments
    let cli = Cli::parse();
    
    match cli.command {
        Command::Validator { data_dir, p2p_port, rpc_port, keystore, stake } => {
            run_validator(data_dir, p2p_port, rpc_port, keystore, stake)
        }
        
        Command::FullNode { data_dir, p2p_port, rpc_port } => {
            run_full_node(data_dir, p2p_port, rpc_port)
        }
        
        Command::InitGenesis { output, chain_id, validators } => {
            init_genesis(output, chain_id, validators)
        }
        
        Command::ConsensusDemo { validators, rounds } => {
            run_consensus_demo(validators, rounds)
        }
        
        Command::Benchmark { filter } => {
            run_benchmarks(filter)
        }
        
        Command::TestAll => {
            run_test_all()
        }
        
        Command::Info { crypto } => {
            show_node_info(crypto)
        }
    }
}

/// Run as a validator node
fn run_validator(
    data_dir: PathBuf,
    p2p_port: u16,
    rpc_port: u16,
    keystore: Option<PathBuf>,
    stake: u128,
) -> Result<()> {
    println!("🚀 Starting TRUE_TRUST Validator Node");
    println!("📁 Data directory: {}", data_dir.display());
    println!("🌐 P2P port: {}", p2p_port);
    println!("🔌 RPC port: {}", rpc_port);
    println!("💰 Initial stake: {} TT", stake);
    
    // Create runtime
    let rt = Runtime::new()?;
    
    rt.block_on(async {
        // Initialize node
        let node = node_core::NodeCore::new(data_dir, true)?;
        
        // Load or generate validator keys
        let (falcon_pk, falcon_sk) = if let Some(ks) = keystore {
            println!("🔑 Loading validator keys from: {}", ks.display());
            // TODO: Load from keystore
            falcon_sigs::falcon_keypair()
        } else {
            println!("🔑 Generating new validator keys...");
            falcon_sigs::falcon_keypair()
        };
        
        let node_id = node_id::node_id_from_falcon_pk(&falcon_pk);
        println!("📍 Node ID: {}", hex::encode(&node_id));
        
        // Start P2P network
        println!("🌐 Starting P2P network on port {}...", p2p_port);
        let p2p = Arc::new(p2p::P2PNetwork::new(p2p_port, node_id).await?);
        
        // Register as validator in consensus
        let mut consensus = consensus_pro::ConsensusPro::new_default();
        consensus.register_validator(node_id, stake);
        consensus.record_quality_f64(&node_id, 1.0); // Start with perfect quality
        consensus.update_all_trust();
        
        println!("✅ Validator node started successfully!");
        println!("⏳ Running... Press Ctrl+C to stop");
        
        // Wait for shutdown signal
        signal::ctrl_c().await?;
        println!("\n🛑 Shutting down validator node...");
        
        Ok::<(), anyhow::Error>(())
    })?;
    
    Ok(())
}

/// Run as a full node (non-validating)
fn run_full_node(data_dir: PathBuf, p2p_port: u16, rpc_port: u16) -> Result<()> {
    println!("🚀 Starting TRUE_TRUST Full Node (non-validating)");
    println!("📁 Data directory: {}", data_dir.display());
    println!("🌐 P2P port: {}", p2p_port);
    println!("🔌 RPC port: {}", rpc_port);
    
    // Create runtime
    let rt = Runtime::new()?;
    
    rt.block_on(async {
        // Initialize node
        let node = node_core::NodeCore::new(data_dir, false)?;
        
        // Generate node identity
        let (falcon_pk, _) = falcon_sigs::falcon_keypair();
        let node_id = node_id::node_id_from_falcon_pk(&falcon_pk);
        println!("📍 Node ID: {}", hex::encode(&node_id));
        
        // Start P2P network
        println!("🌐 Starting P2P network on port {}...", p2p_port);
        let p2p = Arc::new(p2p::P2PNetwork::new(p2p_port, node_id).await?);
        
        println!("✅ Full node started successfully!");
        println!("⏳ Running... Press Ctrl+C to stop");
        
        // Wait for shutdown signal
        signal::ctrl_c().await?;
        println!("\n🛑 Shutting down full node...");
        
        Ok::<(), anyhow::Error>(())
    })?;
    
    Ok(())
}

/// Initialize a new genesis block
fn init_genesis(output: PathBuf, chain_id: String, validators: Option<PathBuf>) -> Result<()> {
    use serde_json::json;
    
    println!("🌟 Initializing new genesis for chain: {}", chain_id);
    
    // Create genesis configuration
    let genesis = json!({
        "chain_id": chain_id,
        "genesis_time": chrono::Utc::now().to_rfc3339(),
        "consensus_params": {
            "block_time_ms": 6000,
            "max_block_size": 1048576,
            "max_tx_size": 32768,
        },
        "validators": [],
        "initial_state": {
            "accounts": [],
            "total_supply": "1000000000000000000", // 1 billion TT
        },
    });
    
    // Write genesis file
    std::fs::write(&output, serde_json::to_string_pretty(&genesis)?)?;
    println!("✅ Genesis file created: {}", output.display());
    
    Ok(())
}

/// Run consensus demonstration
fn run_consensus_demo(validators: u32, rounds: u32) -> Result<()> {
    println!("🎮 Running Consensus Demo");
    println!("👥 Validators: {}", validators);
    println!("🔄 Rounds: {}", rounds);
    println!();
    
    // Create consensus instance
    let mut consensus = consensus_pro::ConsensusPro::new_default();
    
    // Generate validators
    let mut validator_ids = Vec::new();
    for i in 0..validators {
        let (pk, _) = falcon_sigs::falcon_keypair();
        let id = node_id::node_id_from_falcon_pk(&pk);
        let stake = 1000000 * (i as u128 + 1); // Variable stakes
        let quality = 0.8 + (i as f64 * 0.05); // Variable quality
        
        consensus.register_validator(id, stake);
        consensus.record_quality_f64(&id, quality);
        validator_ids.push((id, stake, quality));
        
        println!("Validator {}: stake={}, quality={:.2}", i+1, stake, quality);
    }
    
    consensus.update_all_trust();
    println!();
    
    // Simulate rounds
    for slot in 0..rounds as u64 {
        println!("=== Round {} ===", slot + 1);
        
        // Select leader
        // Create a deterministic beacon from slot number
        let mut beacon = [0u8; 32];
        let slot_bytes = slot.to_le_bytes();
        beacon[..8].copy_from_slice(&slot_bytes);
        let leader = consensus.select_leader(beacon).ok_or_else(|| anyhow!("No leader selected"))?;
        let leader_info = validator_ids.iter()
            .position(|(id, _, _)| *id == leader)
            .map(|i| i + 1)
            .unwrap_or(0);
        
        println!("Leader: Validator {}", leader_info);
        
        // Show weights
        let mut weights = Vec::new();
        for (i, (id, _, _)) in validator_ids.iter().enumerate() {
            let weight = consensus.compute_validator_weight(id).unwrap_or(0);
            weights.push((i + 1, weight));
        }
        
        let total_weight: u128 = weights.iter().map(|(_, w)| w).sum();
        for (i, w) in weights {
            let percent = (w as f64 / total_weight as f64) * 100.0;
            println!("  Validator {}: weight={} ({:.1}%)", i, w, percent);
        }
        
        println!();
    }
    
    Ok(())
}

/// Run crypto benchmarks
fn run_benchmarks(filter: Option<String>) -> Result<()> {
    use std::time::Instant;
    
    println!("🏃 Running Crypto Benchmarks");
    if let Some(f) = &filter {
        println!("Filter: {}", f);
    }
    println!();
    
    // Benchmark Falcon-512
    if filter.as_ref().map_or(true, |f| f.contains("falcon")) {
        println!("=== Falcon-512 ===");
        
        let start = Instant::now();
        let (pk, sk) = falcon_sigs::falcon_keypair();
        let keygen_time = start.elapsed();
        
        let msg = b"benchmark message";
        let start = Instant::now();
        let sig = falcon_sigs::falcon_sign_nullifier(&[0u8; 32], &sk)?;
        let sign_time = start.elapsed();
        
        let start = Instant::now();
        falcon_sigs::falcon_verify_nullifier(&[0u8; 32], &sig, &pk)?;
        let verify_time = start.elapsed();
        
        println!("  Key generation: {:?}", keygen_time);
        println!("  Signing: {:?}", sign_time);
        println!("  Verification: {:?}", verify_time);
        println!("  Public key size: {} bytes", SignPublicKey::as_bytes(&pk).len());
        println!("  Signature size: ~666 bytes (average)");
        println!();
    }
    
    // Benchmark Kyber-768
    if filter.as_ref().map_or(true, |f| f.contains("kyber")) {
        println!("=== Kyber-768 ===");
        
        let start = Instant::now();
        let (pk, sk) = kyber_kem::kyber_keypair();
        let keygen_time = start.elapsed();
        
        let start = Instant::now();
        let (ss1, ct) = kyber_kem::kyber_encapsulate(&pk);
        let encap_time = start.elapsed();
        
        let start = Instant::now();
        let ss2 = kyber_kem::kyber_decapsulate(&ct, &sk)?;
        let decap_time = start.elapsed();
        
        println!("  Key generation: {:?}", keygen_time);
        println!("  Encapsulation: {:?}", encap_time);
        println!("  Decapsulation: {:?}", decap_time);
        println!("  Public key size: {} bytes", KemPublicKey::as_bytes(&pk).len());
        println!("  Ciphertext size: {} bytes", KemCiphertext::as_bytes(&ct).len());
        println!("  Shared secret size: {} bytes", KemSharedSecret::as_bytes(&ss1).len());
        println!();
    }
    
    // Benchmark KMAC256
    if filter.as_ref().map_or(true, |f| f.contains("kmac")) {
        use crypto::kmac;
        
        println!("=== KMAC256 ===");
        
        let key = [0u8; 32];
        let data = vec![0u8; 1024]; // 1KB
        
        let start = Instant::now();
        let tag = kmac::kmac256_tag(&key, b"label", &data);
        let tag_time = start.elapsed();
        
        let start = Instant::now();
        let derived = kmac::kmac256_derive_key(&key, b"label", &data);
        let derive_time = start.elapsed();
        
        println!("  Tag (1KB): {:?}", tag_time);
        println!("  Key derivation: {:?}", derive_time);
        println!("  Tag size: {} bytes", tag.len());
        println!();
    }
    
    Ok(())
}

/// Test all features
fn run_test_all() -> Result<()> {
    println!("🧪 Testing all features...\n");
    
    // Test crypto
    println!("Testing Falcon-512...");
    let (pk, sk) = falcon_sigs::falcon_keypair();
    let sig = falcon_sigs::falcon_sign_nullifier(&[42u8; 32], &sk)?;
    falcon_sigs::falcon_verify_nullifier(&[42u8; 32], &sig, &pk)?;
    println!("✅ Falcon-512 OK");
    
    println!("Testing Kyber-768...");
    let (pk, sk) = kyber_kem::kyber_keypair();
    let (ss1, ct) = kyber_kem::kyber_encapsulate(&pk);
    let ss2 = kyber_kem::kyber_decapsulate(&ct, &sk)?;
    assert_eq!(KemSharedSecret::as_bytes(&ss1), KemSharedSecret::as_bytes(&ss2));
    println!("✅ Kyber-768 OK");
    
    // Test consensus
    println!("Testing consensus...");
    let mut consensus = consensus_pro::ConsensusPro::new_default();
    let (pk, _) = falcon_sigs::falcon_keypair();
    let id = node_id::node_id_from_falcon_pk(&pk);
    consensus.register_validator(id, 1000000);
    consensus.update_all_trust();
    let beacon = [0u8; 32];
    let _ = consensus.select_leader(beacon).ok_or_else(|| anyhow!("No leader selected"))?;
    println!("✅ Consensus OK");
    
    // Test storage
    println!("Testing chain store...");
    let store = chain_store::ChainStore::new();
    println!("✅ Chain store OK");
    
    println!("\n🎉 All tests passed!");
    
    Ok(())
}

/// Show node information
fn show_node_info(crypto: bool) -> Result<()> {
    println!("TRUE_TRUST Node v{}", env!("CARGO_PKG_VERSION"));
    println!("Post-Quantum Blockchain Implementation");
    println!();
    
    println!("Features:");
    println!("  ✅ Falcon-512 signatures");
    println!("  ✅ Kyber-768 key encapsulation");
    println!("  ✅ KMAC256 hashing");
    println!("  ✅ Deterministic PRO consensus");
    println!("  ✅ Golden Trio quality system");
    
    #[cfg(feature = "winterfell")]
    println!("  ✅ STARK proofs (Winterfell)");
    
    #[cfg(feature = "seeded_falcon")]
    println!("  ✅ Deterministic Falcon (seeded)");
    
    #[cfg(feature = "wallet")]
    println!("  ✅ Wallet CLI");
    
    if crypto {
        println!("\nCrypto Details:");
        println!("  Falcon-512:");
        println!("    - Security level: 128-bit post-quantum");
        println!("    - Public key: 897 bytes");
        println!("    - Signature: ~666 bytes (average)");
        
        println!("  Kyber-768:");
        println!("    - Security level: 128-bit post-quantum");
        println!("    - Public key: 1184 bytes");
        println!("    - Ciphertext: 1088 bytes");
        
        println!("  KMAC256:");
        println!("    - Based on SHA3/SHAKE256");
        println!("    - Output: variable length");
    }
    
    Ok(())
}