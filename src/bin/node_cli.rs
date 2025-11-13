//! TT Blockchain Node CLI - Production Node with PoT+PoZS+Bulletproofs+RISC0

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use anyhow::Result;

use std::sync::Arc;
use tokio::sync::Mutex;
use tt_priv_cli::pot::{PotParams, TrustParams, q_from_ratio};
use tt_priv_cli::pot_node::{PotNode, PotNodeConfig, GenesisValidator};
use tt_priv_cli::node::NodeV2;
use tt_priv_cli::consensus::Trust;
use tt_priv_cli::state::State;
use tt_priv_cli::state_priv::StatePriv;
use tt_priv_cli::crypto_kmac_consensus::kmac256_hash;

#[derive(Parser, Debug)]
#[command(name = "tt_node", version, author)]
#[command(about = "TT Blockchain Node - PoT+PoZS+Bulletproofs+RISC0")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Initialize and start blockchain node
    Start {
        /// Node data directory
        #[arg(long, default_value = "./node_data")]
        data_dir: PathBuf,
        
        /// Network listen address
        #[arg(long, default_value = "127.0.0.1:8333")]
        listen: String,
        
        /// Node ID (hex)
        #[arg(long)]
        node_id: Option<String>,
    },
    
    /// Show node status
    Status {
        #[arg(long, default_value = "./node_data")]
        data_dir: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    match cli.cmd {
        Cmd::Start { data_dir, listen, node_id } => {
            println!("🚀 Starting TT Blockchain Node...");
            println!("📁 Data directory: {:?}", data_dir);
            println!("🌐 Listen address: {}", listen);
            
            // Generate or load node ID
            let node_id_bytes = if let Some(id_hex) = node_id {
                hex::decode(&id_hex)?
            } else {
                use rand::RngCore;
                let mut rng = rand::thread_rng();
                let mut id = vec![0u8; 32];
                rng.fill_bytes(&mut id);
                println!("🔑 Generated node ID: {}", hex::encode(&id));
                id
            };
            
            // Initialize PoT parameters
            let trust_params = TrustParams {
                alpha_q: q_from_ratio(95, 100), // 0.95 decay
                beta_q: q_from_ratio(5, 100),   // 0.05 reward
                init_q: q_from_ratio(1, 2),     // 0.5 initial trust
            };
            
            let pot_params = PotParams {
                trust: trust_params,
                lambda_q: q_from_ratio(1, 2),
                min_bond: 1_000_000,
                slash_noreveal_bps: 1000, // 10%
            };
            
            // Genesis validators (single validator for testing)
            let node_id_32: [u8; 32] = node_id_bytes.clone().try_into().unwrap_or([0u8; 32]);
            let genesis_validators = vec![
                GenesisValidator {
                    who: node_id_32,
                    stake: 10_000_000u64,
                    active: true,
                    trust_override: None,
                }
            ];
            
            // Initialize PoT node config
            let pot_config = PotNodeConfig {
                node_id: node_id_32,
                slot_duration: std::time::Duration::from_secs(6),
                epoch_length: 256,
                params: pot_params.clone(),
                equivocation_penalty_bps: 5000, // 50% slash for equivocation
            };
            
            let genesis_beacon = kmac256_hash(b"GENESIS_RANDAO", &[b"TT_BLOCKCHAIN_V1"]);
            let pot_node = PotNode::new(pot_config, genesis_validators, genesis_beacon);
            
            // Initialize state
            std::fs::create_dir_all(&data_dir)?;
            let state_path = data_dir.join("state.json");
            let state = if state_path.exists() {
                State::open(state_path)?
            } else {
                State::new()
            };
            
            let state_priv_path = data_dir.join("state_priv.json");
            let state_priv = if state_priv_path.exists() {
                StatePriv::open(state_priv_path)?
            } else {
                StatePriv::new()
            };
            
            // Create Trust state (simple version)
            let trust = Trust::new();
            
            // Create blockchain node (takes ownership of pot_node, state, state_priv)
            let node = NodeV2::new(
                Some(listen.clone()),
                pot_node,
                state,
                state_priv,
                trust,
            );
            let node_arc = Arc::new(node);
            
            // Initialize Bloom filters
            node_arc.init_filters(data_dir.join("filters")).await?;
            
            // Spawn network listener
            let node_clone = node_arc.clone();
            tokio::spawn(async move {
                if let Err(e) = node_clone.run().await {
                    eprintln!("Node run error: {}", e);
                }
            });
            
            // Start mining loop
            let node_mining = node_arc.clone();
            let seed = node_id_32;
            tokio::spawn(async move {
                if let Err(e) = node_mining.mine_loop(0, 6, seed).await {
                    eprintln!("⛏️  Mining error: {}", e);
                }
            });
            
            println!("✅ Node started successfully!");
            println!("📡 Listening on {}", listen);
            println!("⛏️  Mining enabled");
            
            // Keep running
            tokio::signal::ctrl_c().await?;
            println!("\n🛑 Shutting down...");
            
            Ok(())
        }
        
        Cmd::Status { data_dir } => {
            println!("📊 Node Status:");
            println!("📁 Data directory: {:?}", data_dir);
            
            // Load and display state
            if data_dir.join("state.json").exists() {
                let state = tt_priv_cli::state::State::open(data_dir.join("state.json"))?;
                println!("💰 Balances: {} accounts", state.balances.len());
                println!("🤝 Trust: {} validators", state.trust.len());
            } else {
                println!("⚠️  No state file found (node not initialized)");
            }
            
            Ok(())
        }
    }
}
