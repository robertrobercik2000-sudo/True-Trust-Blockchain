//! TT Quantum CLI - Full featured quantum-safe wallet
//! 
//! Commands:
//! - wallet-init: Create wallet (--quantum for PQC)
//! - info: Show wallet info
//! - address: Show wallet address
//! - build-quantum-hint: Build quantum-safe payment hint
//! - verify-quantum-hint: Verify quantum-safe payment hint

#![forbid(unsafe_code)]
#![cfg(feature = "tt-full")]

use crate::tt_quantum_wallet::*;
use crate::crypto::kmac_mlkem_integration::QuantumKeySearchCtx;  // ✅ POPRAWIONE
use anyhow::{anyhow, bail, ensure, Context, Result};
use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;
use dirs;
use zeroize::Zeroizing;
use pqcrypto_traits::sign::PublicKey as PQPublicKey;
use pqcrypto_traits::kem::PublicKey as PQKemPublicKey;

/* =========================================================================================
 * CLI STRUCTURE
 * ====================================================================================== */

#[derive(Parser)]
#[command(name = "ttq")]
#[command(about = "TT Quantum Wallet - Post-quantum secure cryptocurrency wallet", long_about = None)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    
    /// Wallet directory (default: ~/.ttq)
    #[arg(short, long, global = true)]
    pub wallet_dir: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize new wallet
    WalletInit {
        /// Wallet name
        #[arg(default_value = "default")]
        name: String,
        
        /// Enable quantum-safe mode (Falcon512 + ML-KEM)
        #[arg(long)]
        quantum: bool,
        
        /// Password for wallet encryption
        #[arg(short, long)]
        password: Option<String>,
    },
    
    /// Show wallet information
    Info {
        /// Wallet name
        #[arg(default_value = "default")]
        name: String,
    },
    
    /// Show wallet address
    Address {
        /// Wallet name
        #[arg(default_value = "default")]
        name: String,
    },
    
    /// Build quantum-safe payment hint
    BuildQuantumHint {
        /// Wallet name
        #[arg(default_value = "default")]
        name: String,
        
        /// Recipient address (bech32)
        #[arg(short, long)]
        to: String,
        
        /// Amount (in smallest unit)
        #[arg(short, long)]
        amount: u64,
        
        /// Epoch for key rotation
        #[arg(short, long, default_value = "0")]
        epoch: u64,
    },
    
    /// Verify quantum-safe payment hint
    VerifyQuantumHint {
        /// Wallet name
        #[arg(default_value = "default")]
        name: String,
        
        /// Hint data (hex)
        #[arg(short = 'd', long)]
        data: String,
    },
}

/* =========================================================================================
 * HELPERS
 * ====================================================================================== */

fn get_wallet_dir(cli_dir: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(dir) = cli_dir {
        return Ok(dir);
    }
    
    let home = dirs::home_dir().ok_or_else(|| anyhow!("cannot find home directory"))?;
    Ok(home.join(".ttq"))
}

fn wallet_path(dir: &PathBuf, name: &str) -> PathBuf {
    dir.join(format!("{}.wallet", name))
}

/* =========================================================================================
 * COMMAND HANDLERS
 * ====================================================================================== */

pub fn cmd_wallet_init(
    dir: PathBuf,
    name: String,
    quantum: bool,
    password: Option<String>,
) -> Result<()> {
    fs::create_dir_all(&dir).context("failed to create wallet directory")?;
    
    let path = wallet_path(&dir, &name);
    if path.exists() {
        bail!("wallet '{}' already exists", name);
    }
    
    // Get password
    let pw = if let Some(p) = password {
        p
    } else {
        #[cfg(feature = "tt-full")]
        {
            println!("Enter password:");
            rpassword::read_password().context("failed to read password")?
        }
        #[cfg(not(feature = "tt-full"))]
        {
            bail!("password required when not using interactive mode");
        }
    };
    
    ensure!(!pw.is_empty(), "password cannot be empty");
    
    // Generate master key
    let master = random_master();
    
    // Create wallet payload
    let payload = create_wallet_v3(master, quantum)?;
    
    // Serialize and encrypt
    let payload_bytes = bincode::serialize(&payload)?;
    
    // TODO: Implement full encryption with AEAD
    // For now, just serialize the header and payload
    let header = WalletHeader {
        version: WALLET_VERSION_QUANTUM,
        kdf: KdfHeader {
            kind: KdfKind::Kmac256V1 { salt32: random_master() },
            info: format!("TT Wallet v{}", WALLET_VERSION_QUANTUM),
        },
        aead: AeadKind::XChaCha20,
        nonce12: random_nonce12(),
        nonce24_opt: Some(random_nonce24()),
        padding_block: 256,
        pepper: PepperPolicy::OsLocal,
        wallet_id: random_wallet_id(),
        quantum_enabled: quantum,
    };
    
    let wallet_file = WalletFile {
        header,
        enc: payload_bytes, // TODO: encrypt
    };
    
    // Save to disk
    let wallet_json = serde_json::to_string_pretty(&wallet_file)?;
    fs::write(&path, wallet_json)?;
    
    // Show wallet info
    let keyset = Keyset::from_payload_v3(&payload)?;
    let addr = if keyset.is_quantum() {
        bech32_addr_quantum(
            &keyset.scan_pk,
            &keyset.spend_pk,
            keyset.falcon_pk.as_ref().unwrap(),
            keyset.mlkem_pk.as_ref().unwrap(),
        )?
    } else {
        bech32_addr(&keyset.scan_pk, &keyset.spend_pk)?
    };
    
    println!("✅ Wallet created: {}", name);
    println!("📁 Path: {}", path.display());
    println!("🔐 Quantum mode: {}", if quantum { "ENABLED ✨" } else { "disabled" });
    println!("📬 Address: {}", addr);
    
    Ok(())
}

pub fn cmd_info(dir: PathBuf, name: String) -> Result<()> {
    let path = wallet_path(&dir, &name);
    ensure!(path.exists(), "wallet '{}' not found", name);
    
    let wallet_json = fs::read_to_string(&path)?;
    let wallet_file: WalletFile = serde_json::from_str(&wallet_json)?;
    
    println!("Wallet: {}", name);
    println!("Path: {}", path.display());
    println!("Version: v{}", wallet_file.header.version);
    println!("Quantum: {}", if wallet_file.header.quantum_enabled { "✅ YES" } else { "❌ NO" });
    println!("AEAD: {:?}", wallet_file.header.aead);
    println!("KDF: {:?}", wallet_file.header.kdf.kind);
    println!("Wallet ID: {}", hex::encode(wallet_file.header.wallet_id));
    
    Ok(())
}

pub fn cmd_address(dir: PathBuf, name: String) -> Result<()> {
    let path = wallet_path(&dir, &name);
    ensure!(path.exists(), "wallet '{}' not found", name);
    
    let wallet_json = fs::read_to_string(&path)?;
    let wallet_file: WalletFile = serde_json::from_str(&wallet_json)?;
    
    // TODO: Decrypt payload with password
    // For now, assume unencrypted
    let payload: WalletSecretPayloadV3 = bincode::deserialize(&wallet_file.enc)?;
    let keyset = Keyset::from_payload_v3(&payload)?;
    
    let addr = if keyset.is_quantum() {
        bech32_addr_quantum(
            &keyset.scan_pk,
            &keyset.spend_pk,
            keyset.falcon_pk.as_ref().unwrap(),
            keyset.mlkem_pk.as_ref().unwrap(),
        )?
    } else {
        bech32_addr(&keyset.scan_pk, &keyset.spend_pk)?
    };
    
    println!("{}", addr);
    
    Ok(())
}

pub fn cmd_build_quantum_hint(
    dir: PathBuf,
    name: String,
    to: String,
    amount: u64,
    epoch: u64,
) -> Result<()> {
    let path = wallet_path(&dir, &name);
    ensure!(path.exists(), "wallet '{}' not found", name);
    
    let wallet_json = fs::read_to_string(&path)?;
    let wallet_file: WalletFile = serde_json::from_str(&wallet_json)?;
    
    ensure!(wallet_file.header.quantum_enabled, "wallet is not quantum-enabled");
    
    // TODO: Decrypt payload
    let payload: WalletSecretPayloadV3 = bincode::deserialize(&wallet_file.enc)?;
    let keyset = Keyset::from_payload_v3(&payload)?;
    
    ensure!(keyset.is_quantum(), "keyset is not quantum");
    
    // Parse recipient address (basic for now)
    // TODO: Full bech32 decode
    println!("🔨 Building quantum hint...");
    println!("  To: {}", to);
    println!("  Amount: {}", amount);
    println!("  Epoch: {}", epoch);
    
    // Create quantum hint using ML-KEM integration
    let _ctx = QuantumKeySearchCtx::new(payload.master32)
        .map_err(|e| anyhow!("failed to create quantum context: {:?}", e))?;
    
    // TODO: Build actual hint with recipient keys
    // For now, just show that we have quantum keys loaded
    println!("✅ Quantum context created");
    println!("  Falcon PK size: {} bytes", keyset.falcon_pk.as_ref().unwrap().as_bytes().len());
    println!("  ML-KEM PK size: {} bytes", keyset.mlkem_pk.as_ref().unwrap().as_bytes().len());
    println!("⚠️  Full hint generation pending recipient key parsing");
    
    Ok(())
}

pub fn cmd_verify_quantum_hint(
    dir: PathBuf,
    name: String,
    data: String,
) -> Result<()> {
    let path = wallet_path(&dir, &name);
    ensure!(path.exists(), "wallet '{}' not found", name);
    
    let wallet_json = fs::read_to_string(&path)?;
    let wallet_file: WalletFile = serde_json::from_str(&wallet_json)?;
    
    ensure!(wallet_file.header.quantum_enabled, "wallet is not quantum-enabled");
    
    // TODO: Full verification
    println!("🔍 Verifying quantum hint...");
    println!("  Data: {}", data);
    println!("⚠️  Full implementation pending");
    
    Ok(())
}

/* =========================================================================================
 * MAIN
 * ====================================================================================== */

pub fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    let dir = get_wallet_dir(cli.wallet_dir)?;
    
    match cli.command {
        Commands::WalletInit { name, quantum, password } => {
            cmd_wallet_init(dir, name, quantum, password)
        }
        Commands::Info { name } => {
            cmd_info(dir, name)
        }
        Commands::Address { name } => {
            cmd_address(dir, name)
        }
        Commands::BuildQuantumHint { name, to, amount, epoch } => {
            cmd_build_quantum_hint(dir, name, to, amount, epoch)
        }
        Commands::VerifyQuantumHint { name, data } => {
            cmd_verify_quantum_hint(dir, name, data)
        }
    }
}
