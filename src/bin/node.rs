//! TRUE_TRUST Blockchain Node CLI
//!
//! Demonstrates integration of:
//! - PoT consensus (pot.rs + pot_node.rs)
//! - PoZS zkSNARK proofs (pozs.rs + pozs_groth16.rs)
//! - Post-quantum wallet (Falcon512 + ML-KEM-768)

use clap::{Parser, Subcommand};
use std::path::PathBuf;

// Import from tt_priv_cli library
use tt_priv_cli::pot::{PotParams, NodeId};
use tt_priv_cli::pot_node::{PotNode, PotNodeConfig, GenesisValidator};

// Post-quantum crypto
use pqcrypto_falcon::falcon512;
use pqcrypto_traits::sign::{PublicKey as _};

#[derive(Parser)]
#[command(name = "tt-node")]
#[command(about = "TRUE_TRUST blockchain node - PoT+PoZS+PQ", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize new validator wallet
    InitValidator,

    /// Run blockchain node
    Run {
        #[arg(long, default_value = "127.0.0.1:8000")]
        listen: String,
        
        #[arg(long)]
        stake: Option<u64>,
    },

    /// Show node info
    Info,

    /// Generate ZK proof for leader eligibility (requires zk-proofs feature)
    #[cfg(feature = "zk-proofs")]
    ProveEligibility {
        #[arg(long)]
        slot: u64,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Simple logging (tracing removed to avoid extra dependencies)
    eprintln!("🚀 TRUE_TRUST Node starting...");

    let cli = Cli::parse();

    match cli.command {
        Commands::InitValidator => {
            init_validator().await?;
        }
        Commands::Run { listen, stake } => {
            run_node(listen, stake).await?;
        }
        Commands::Info => {
            show_info().await?;
        }
        #[cfg(feature = "zk-proofs")]
        Commands::ProveEligibility { slot } => {
            prove_eligibility(slot).await?;
        }
    }

    Ok(())
}

async fn init_validator() -> anyhow::Result<()> {
    println!("🔐 Initializing TRUE_TRUST validator...\n");

    // Generate Falcon512 keypair
    let (falcon_pk, falcon_sk) = falcon512::keypair();
    println!("✓ Generated Falcon512 keypair");
    println!("  Public key size: {} bytes", falcon_pk.as_bytes().len());

    // Derive NodeId from public key
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"NODE_ID.v1");
    h.update(falcon_pk.as_bytes());
    let node_id_hash = h.finalize();
    let mut node_id = [0u8; 32];
    node_id.copy_from_slice(&node_id_hash);

    println!("✓ Node ID: {}", hex::encode(node_id));

    // Generate Kyber768 keypair for KEM
    let (_kyber_pk, _kyber_sk) = pqcrypto_kyber::kyber768::keypair();
    println!("✓ Generated ML-KEM-768 (Kyber) keypair");
    println!("  Public key size: ~1184 bytes");

    println!("\n📁 Wallet location: ~/.tt_wallet/ (NOT IMPLEMENTED in this demo)");
    println!("⚠️  In production: use encrypted storage with Argon2id KDF!");

    Ok(())
}

async fn run_node(listen: String, stake: Option<u64>) -> anyhow::Result<()> {
    println!("🚀 Starting TRUE_TRUST blockchain node...\n");
    println!("Listen address: {}", listen);

    // Create PoT node configuration
    let node_id = [1u8; 32]; // Demo ID
    use tt_priv_cli::pot::{TrustParams, ONE_Q};
    let config = PotNodeConfig {
        node_id,
        slot_duration: std::time::Duration::from_secs(6),
        epoch_length: 32,
        params: PotParams {
            trust: TrustParams {
                alpha_q: ONE_Q / 100, // decay rate
                beta_q: ONE_Q / 100,   // reward rate  
                init_q: ONE_Q,         // initial trust
            },
            lambda_q: 429496729, // ~0.1 in Q32.32 (10% leader ratio)
            min_bond: 100_000,
            slash_noreveal_bps: 1000,
        },
        equivocation_penalty_bps: 5000, // 50% slash
    };

    println!("✓ PoT consensus configured");
    println!("  Epoch length: 32 slots");
    println!("  Slot duration: 6 seconds");
    println!("  Leader ratio: ~10%");

    // Create PoT node with genesis validators
    let genesis_validators = if let Some(stake_amount) = stake {
        vec![GenesisValidator {
            who: node_id,
            stake: stake_amount,
            active: true,
            trust_override: None,
        }]
    } else {
        vec![]
    };

    let genesis_beacon = [0u8; 32]; // Initial beacon
    let _pot_node = PotNode::new(config, genesis_validators.clone(), genesis_beacon);

    if stake.is_some() {
        println!("✓ Registered as validator with stake: {}", genesis_validators[0].stake);
    }

    println!("\n📊 Node Status:");
    println!("  Node ID: {}", hex::encode(node_id));
    println!("  PoT: ✅ Ready");

    #[cfg(feature = "zk-proofs")]
    println!("  PoZS: ✅ Groth16/BN254 enabled");

    #[cfg(not(feature = "zk-proofs"))]
    println!("  PoZS: ⚠️  Disabled (compile with --features zk-proofs)");

    println!("\n🔄 Node loop running... (Ctrl+C to stop)");
    println!("   NOTE: Full P2P networking not implemented in this demo");
    println!("   See src/blockchain_node.rs for full implementation (work in progress)");

    // Simple event loop
    tokio::signal::ctrl_c().await?;
    println!("\n✓ Node stopped");

    Ok(())
}

async fn show_info() -> anyhow::Result<()> {
    println!("=== TRUE_TRUST Node Info ===\n");
    
    println!("📦 Consensus:");
    println!("  Type: Proof-of-Trust (PoT)");
    println!("  RANDAO: ✅ Commit-reveal beacon");
    println!("  Merkle snapshots: ✅ Per-epoch weights");
    println!("  Leader selection: Sortition-based");
    
    println!("\n🔐 Post-Quantum Cryptography:");
    println!("  Signature: Falcon512 (NIST PQC)");
    println!("  KEM: ML-KEM-768 (Kyber)");
    
    println!("\n⚡ ZK Proofs:");
    #[cfg(feature = "zk-proofs")]
    {
        println!("  Status: ✅ Enabled");
        println!("  System: Groth16 over BN254");
        println!("  Proof size: ~128 bytes");
        println!("  Circuit: Leader eligibility verification");
    }
    #[cfg(not(feature = "zk-proofs"))]
    {
        println!("  Status: ⚠️  Disabled");
        println!("  Enable with: cargo build --features zk-proofs");
    }

    println!("\n📚 Modules:");
    println!("  pot.rs: {} lines (consensus core)", count_lines("src/pot.rs")?);
    println!("  pot_node.rs: {} lines (node runtime)", count_lines("src/pot_node.rs")?);
    println!("  pozs.rs: {} lines (ZK integration)", count_lines("src/pozs.rs")?);
    #[cfg(feature = "zk-proofs")]
    println!("  pozs_groth16.rs: {} lines (Groth16 circuit)", count_lines("src/pozs_groth16.rs")?);

    Ok(())
}

#[cfg(feature = "zk-proofs")]
async fn prove_eligibility(slot: u64) -> anyhow::Result<()> {
    use tt_priv_cli::pozs_groth16::{
        setup_keys, prove_eligibility as prove,
        EligibilityCircuit, EligibilityPublicInputs, EligibilityWitness,
    };

    println!("🔐 Generating ZK proof for slot {}...\n", slot);

    // Example public inputs
    let public_inputs = EligibilityPublicInputs {
        weights_root: [1u8; 32],
        beacon_value: [2u8; 32],
        threshold_q: 1000,
        sum_weights_q: 10000,
    };

    // Example private witness
    let witness = EligibilityWitness {
        who: [3u8; 32],
        slot,
        stake_q: 5000,
        trust_q: 100,
        merkle_siblings: vec![],
        leaf_index: 0,
    };

    println!("⏳ Setting up Groth16 keys (one-time)...");
    use rand::rngs::OsRng;
    let mut rng = OsRng;
    let (pk, _vk) = setup_keys(&mut rng).map_err(|e| anyhow::anyhow!("{:?}", e))?;
    println!("✓ Keys generated");

    println!("⏳ Generating proof...");
    let proof = prove(&pk, &public_inputs, &witness, &mut rng).map_err(|e| anyhow::anyhow!("{:?}", e))?;
    println!("✓ Proof generated");

    // Serialize proof
    use tt_priv_cli::pozs_groth16::serialize_proof;
    let proof_bytes = serialize_proof(&proof).map_err(|e| anyhow::anyhow!("{:?}", e))?;
    println!("\n📊 Proof Statistics:");
    println!("  Size: {} bytes", proof_bytes.len());
    println!("  Hex: {}...", hex::encode(&proof_bytes[..32.min(proof_bytes.len())]));

    println!("\n✅ ZK proof generation successful!");
    println!("   This proves leader eligibility WITHOUT revealing:");
    println!("   - Validator identity (who)");
    println!("   - Stake amount (stake_q)");
    println!("   - Trust score (trust_q)");

    Ok(())
}

fn count_lines(path: &str) -> anyhow::Result<usize> {
    use std::fs;
    let content = fs::read_to_string(path)?;
    Ok(content.lines().count())
}
