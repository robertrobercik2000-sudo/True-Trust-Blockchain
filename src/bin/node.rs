//! TT Blockchain Node CLI - Production version with Consensus PRO v2
//!
//! Uses NEW modules:
//! - consensus_pro_v2: Deterministic consensus (Q32.32)
//! - rtt_pro: RTT PRO (no floats)
//! - node_id: NodeId from Falcon PK
//! - p2p_channel: Secure P2P channel

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use anyhow::Result;

use tt_priv_cli::consensus_pro_v2::ConsensusPro;
use tt_priv_cli::node_id::{NodeId, node_id_from_falcon_pk};
use tt_priv_cli::rtt_pro::{q_from_f64, Q, ONE_Q};

// PQ crypto
use pqcrypto_falcon::falcon512;
use pqcrypto_traits::sign::{PublicKey as PQPublicKey, SecretKey as PQSecretKey};

#[derive(Parser, Debug)]
#[command(name = "tt_node", version, author)]
#[command(about = "TRUE_TRUST Node - Consensus PRO v2 (PQ + Q32.32)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Initialize new validator node
    Init {
        /// Node data directory
        #[arg(long, default_value = "./node_data")]
        data_dir: PathBuf,
        
        /// Initial stake (raw units)
        #[arg(long, default_value_t = 10_000_000)]
        stake: u128,
    },
    
    /// Run consensus (simplified demo)
    Run {
        /// Node data directory
        #[arg(long, default_value = "./node_data")]
        data_dir: PathBuf,
        
        /// Network listen address
        #[arg(long, default_value = "127.0.0.1:8333")]
        listen: String,
    },
    
    /// Show validator status
    Status {
        #[arg(long, default_value = "./node_data")]
        data_dir: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    match cli.cmd {
        Cmd::Init { data_dir, stake } => {
            cmd_init(data_dir, stake).await
        }
        
        Cmd::Run { data_dir, listen } => {
            cmd_run(data_dir, listen).await
        }
        
        Cmd::Status { data_dir } => {
            cmd_status(data_dir).await
        }
    }
}

async fn cmd_init(data_dir: PathBuf, stake: u128) -> Result<()> {
    println!("🔧 Initializing validator node...");
    println!("📁 Data directory: {:?}", data_dir);
    
    std::fs::create_dir_all(&data_dir)?;
    
    // Generate Falcon keypair
    let (falcon_pk, falcon_sk) = falcon512::keypair();
    let node_id = node_id_from_falcon_pk(&falcon_pk);
    
    println!("🔑 Node ID: {}", hex::encode(&node_id));
    println!("💰 Initial stake: {}", stake);
    
    // Save keys (SIMPLIFIED - in production, encrypt this!)
    let keys_path = data_dir.join("validator.keys");
    let keys_json = serde_json::json!({
        "node_id": hex::encode(&node_id),
        "falcon_pk": hex::encode(falcon_pk.as_bytes()),
        "falcon_sk": hex::encode(falcon_sk.as_bytes()),
        "stake": stake,
    });
    std::fs::write(keys_path, serde_json::to_string_pretty(&keys_json)?)?;
    
    println!("✅ Validator initialized!");
    println!("⚠️  WARNING: Keys stored UNENCRYPTED (demo only)");
    
    Ok(())
}

async fn cmd_run(data_dir: PathBuf, listen: String) -> Result<()> {
    println!("🚀 Starting TT Blockchain Node (Consensus PRO v2)...");
    println!("📁 Data directory: {:?}", data_dir);
    println!("🌐 Listen address: {}", listen);
    
    // Load validator keys
    let keys_path = data_dir.join("validator.keys");
    if !keys_path.exists() {
        eprintln!("❌ No validator keys found. Run 'tt_node init' first.");
        std::process::exit(1);
    }
    
    let keys_json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(keys_path)?)?;
    let node_id_hex = keys_json["node_id"].as_str().unwrap();
    let node_id: NodeId = hex::decode(node_id_hex)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid node_id"))?;
    let stake: u128 = keys_json["stake"].as_u64().unwrap_or(10_000_000) as u128;
    
    println!("🔑 Node ID: {}", hex::encode(&node_id));
    
    // Initialize Consensus PRO v2
    let mut consensus = ConsensusPro::new_default();
    
    // Register this validator
    consensus.register_validator(node_id, stake);
    consensus.recompute_all_stake_q();
    
    // Simulate some quality (Golden Trio would compute this from actual work)
    consensus.record_quality(&node_id, q_from_f64(0.8));
    consensus.update_validator_trust(&node_id);
    
    // Show validator state
    if let Some(v) = consensus.get_validator(&node_id) {
        use tt_priv_cli::rtt_pro::q_to_f64;
        println!("📊 Validator state:");
        println!("   Trust:   {:.4}", q_to_f64(v.trust_q));
        println!("   Quality: {:.4}", q_to_f64(v.quality_q));
        println!("   Stake:   {:.4}", q_to_f64(v.stake_q));
        println!("   Weight:  {:?}", consensus.compute_validator_weight(&node_id));
    }
    
    // Simulate leader selection
    let beacon = [0x42u8; 32]; // In production: from RandomX
    if let Some(leader) = consensus.select_leader(beacon) {
        if leader == node_id {
            println!("👑 THIS NODE is selected as leader!");
        } else {
            println!("📡 Leader: {}", hex::encode(&leader[..8]));
        }
    }
    
    println!("\n✅ Consensus PRO v2 demo complete!");
    println!("📌 Uses: RTT PRO (Q32.32) + deterministic weights");
    println!("⚠️  Network layer NOT implemented (P2P incomplete)");
    
    Ok(())
}

async fn cmd_status(data_dir: PathBuf) -> Result<()> {
    println!("📊 Node Status:");
    println!("📁 Data directory: {:?}", data_dir);
    
    let keys_path = data_dir.join("validator.keys");
    if keys_path.exists() {
        let keys_json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(keys_path)?)?;
        println!("🔑 Node ID: {}", keys_json["node_id"].as_str().unwrap());
        println!("💰 Stake: {}", keys_json["stake"]);
    } else {
        println!("⚠️  No validator keys found");
    }
    
    Ok(())
}
