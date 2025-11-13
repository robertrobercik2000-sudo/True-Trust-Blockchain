//! TRUE_TRUST Blockchain Node CLI

use tt_blockchain_node::node::{BlockchainNode, NodeConfig};
use tt_blockchain_node::wallet::{PqWallet, WalletConfig};
use tt_blockchain_node::consensus::{Registry, ValidatorInfo};
use clap::{Parser, Subcommand};
use anyhow::{Result, Context};
use tracing::info;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "tt-node")]
#[command(about = "TRUE_TRUST blockchain node - PoT+PoZS+PQ", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize new wallet
    InitWallet {
        #[arg(long, default_value = ".tt_wallet")]
        wallet_dir: PathBuf,
    },

    /// Run blockchain node
    Run {
        #[arg(long, default_value = ".tt_wallet")]
        wallet_dir: PathBuf,
        
        #[arg(long, default_value = "./data")]
        data_dir: PathBuf,
        
        #[arg(long, default_value = "127.0.0.1:8000")]
        listen: String,
        
        #[arg(long)]
        bootstrap: Vec<String>,
        
        #[arg(long)]
        stake: Option<u128>,
    },

    /// Show node info
    Info {
        #[arg(long, default_value = ".tt_wallet")]
        wallet_dir: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into())
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::InitWallet { wallet_dir } => {
            init_wallet(wallet_dir).await?;
        }
        Commands::Run { wallet_dir, data_dir, listen, bootstrap, stake } => {
            run_node(wallet_dir, data_dir, listen, bootstrap, stake).await?;
        }
        Commands::Info { wallet_dir } => {
            show_info(wallet_dir).await?;
        }
    }

    Ok(())
}

async fn init_wallet(wallet_dir: PathBuf) -> Result<()> {
    let config = WalletConfig { wallet_dir };
    let mut wallet = PqWallet::new(config.clone());

    if wallet.exists() {
        anyhow::bail!("Wallet already exists at {:?}", config.wallet_dir);
    }

    println!("Creating new post-quantum wallet...");
    let password = rpassword::prompt_password("Enter password: ")
        .context("Failed to read password")?;
    let confirm = rpassword::prompt_password("Confirm password: ")
        .context("Failed to read password")?;

    if password != confirm {
        anyhow::bail!("Passwords do not match");
    }

    wallet.create(&password)?;

    let node_id = wallet.node_id()?;
    println!("✓ Wallet created successfully!");
    println!("  Node ID: {}", hex::encode(node_id));
    println!("  Location: {:?}", config.wallet_dir);
    println!("\n⚠️  Keep your password safe - it cannot be recovered!");

    Ok(())
}

async fn run_node(
    wallet_dir: PathBuf,
    data_dir: PathBuf,
    listen: String,
    bootstrap: Vec<String>,
    stake: Option<u128>,
) -> Result<()> {
    info!("Starting TRUE_TRUST blockchain node...");

    // Load wallet
    let config = WalletConfig { wallet_dir };
    let mut wallet = PqWallet::new(config.clone());

    if !wallet.exists() {
        anyhow::bail!("Wallet not found. Run 'init-wallet' first.");
    }

    let password = rpassword::prompt_password("Enter wallet password: ")
        .context("Failed to read password")?;
    
    wallet.unlock(&password)?;
    let node_id = wallet.node_id()?;
    info!("Node ID: {}", hex::encode(node_id));

    // Create node config
    let node_config = NodeConfig {
        data_dir,
        listen_addr: listen,
        bootstrap_peers: bootstrap,
        ..Default::default()
    };

    // Create and initialize node
    let mut node = BlockchainNode::new(node_config, wallet)?;

    // Initialize ZK system if feature is enabled
    #[cfg(feature = "zk-proofs")]
    {
        info!("ZK proofs feature enabled - initializing Groth16...");
        node.init_zk()?;
    }

    // Register as validator if stake provided
    if let Some(stake_amount) = stake {
        info!("Registering as validator with stake: {}", stake_amount);
        let mut registry = node.registry.write().await;
        registry.register(ValidatorInfo {
            node_id,
            stake: stake_amount,
            public_key: vec![], // TODO: Extract from wallet
            network_addr: node.config.listen_addr.clone(),
        });
    }

    // Start node
    info!("Node starting...");
    node.start().await?;

    Ok(())
}

async fn show_info(wallet_dir: PathBuf) -> Result<()> {
    let config = WalletConfig { wallet_dir };
    let mut wallet = PqWallet::new(config.clone());

    if !wallet.exists() {
        anyhow::bail!("Wallet not found at {:?}", config.wallet_dir);
    }

    let password = rpassword::prompt_password("Enter wallet password: ")
        .context("Failed to read password")?;
    
    wallet.unlock(&password)?;
    let node_id = wallet.node_id()?;

    println!("=== TRUE_TRUST Node Info ===");
    println!("Node ID: {}", hex::encode(node_id));
    println!("Wallet: {:?}", config.wallet_dir);
    println!("\nPost-Quantum Algorithms:");
    println!("  Signature: Falcon512");
    println!("  KEM: ML-KEM-768 (Kyber)");
    
    #[cfg(feature = "zk-proofs")]
    println!("\nZK Proofs: Enabled (Groth16/BN254)");
    
    #[cfg(not(feature = "zk-proofs"))]
    println!("\nZK Proofs: Disabled");

    Ok(())
}
