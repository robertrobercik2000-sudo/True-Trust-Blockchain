//! Quantum Falcon Wallet CLI
//! 
//! Post-quantum secure cryptocurrency wallet with Falcon512 + X25519
#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};
use quantum_falcon_wallet::*;
use rand::RngCore;
use std::fs;
use std::path::PathBuf;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret as X25519StaticSecret};

/* ===== CLI STRUCTURE ===== */

#[derive(Parser)]
#[command(name = "qfw")]
#[command(about = "Quantum Falcon Wallet - Post-quantum secure cryptocurrency wallet", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    /// Wallet directory (default: ~/.qfw)
    #[arg(short, long, global = true)]
    wallet_dir: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate new wallet keys
    Keygen {
        /// Wallet name
        #[arg(default_value = "default")]
        name: String,
        
        /// Use quantum-safe Falcon512
        #[arg(long)]
        quantum: bool,
    },
    
    /// Display wallet information
    Info {
        /// Wallet name
        #[arg(default_value = "default")]
        name: String,
    },
    
    /// Build payment hint
    Send {
        /// Recipient's public key (hex)
        #[arg(short, long)]
        to: String,
        
        /// Amount to send
        #[arg(short, long)]
        amount: u64,
        
        /// Memo text
        #[arg(short, long)]
        memo: Option<String>,
        
        /// Use quantum-safe hint
        #[arg(long)]
        quantum: bool,
    },
    
    /// Scan for received payments
    Receive {
        /// Wallet name
        #[arg(default_value = "default")]
        name: String,
        
        /// Hint data (hex)
        #[arg(short, long)]
        hint: String,
        
        /// C_out commitment (hex)
        #[arg(short, long)]
        c_out: String,
    },
    
    /// Export public keys
    Export {
        /// Wallet name
        #[arg(default_value = "default")]
        name: String,
        
        /// Export format (hex/json)
        #[arg(short, long, default_value = "hex")]
        format: String,
    },
}

/* ===== WALLET STORAGE ===== */

struct WalletKeys {
    name: String,
    master_seed: [u8; 32],
    view_secret: [u8; 32],
    quantum_enabled: bool,
}

impl WalletKeys {
    fn new(name: String, quantum_enabled: bool) -> Self {
        let mut master_seed = [0u8; 32];
        let mut view_secret = [0u8; 32];
        
        rand::rngs::OsRng.fill_bytes(&mut master_seed);
        rand::rngs::OsRng.fill_bytes(&mut view_secret);
        
        Self {
            name,
            master_seed,
            view_secret,
            quantum_enabled,
        }
    }
    
    fn save(&self, wallet_dir: &PathBuf) -> anyhow::Result<()> {
        fs::create_dir_all(wallet_dir)?;
        
        let wallet_file = wallet_dir.join(format!("{}.wallet", self.name));
        let data = serde_json::json!({
            "name": self.name,
            "master_seed": hex::encode(self.master_seed),
            "view_secret": hex::encode(self.view_secret),
            "quantum_enabled": self.quantum_enabled,
        });
        
        fs::write(wallet_file, serde_json::to_string_pretty(&data)?)?;
        println!("✓ Wallet saved: {}", self.name);
        Ok(())
    }
    
    fn load(name: &str, wallet_dir: &PathBuf) -> anyhow::Result<Self> {
        let wallet_file = wallet_dir.join(format!("{}.wallet", name));
        let data = fs::read_to_string(wallet_file)?;
        let json: serde_json::Value = serde_json::from_str(&data)?;
        
        let master_seed = hex::decode(json["master_seed"].as_str().unwrap())?;
        let view_secret = hex::decode(json["view_secret"].as_str().unwrap())?;
        
        let mut ms = [0u8; 32];
        let mut vs = [0u8; 32];
        ms.copy_from_slice(&master_seed);
        vs.copy_from_slice(&view_secret);
        
        Ok(Self {
            name: json["name"].as_str().unwrap().to_string(),
            master_seed: ms,
            view_secret: vs,
            quantum_enabled: json["quantum_enabled"].as_bool().unwrap_or(false),
        })
    }
}

/* ===== COMMAND HANDLERS ===== */

fn cmd_keygen(name: String, quantum: bool, wallet_dir: &PathBuf) -> anyhow::Result<()> {
    println!("🔑 Generating new wallet: {}", name);
    println!("   Quantum-safe: {}", if quantum { "✓ Enabled" } else { "✗ Disabled" });
    
    let wallet = WalletKeys::new(name, quantum);
    wallet.save(wallet_dir)?;
    
    // Display public keys
    if quantum {
        let ctx = QuantumKeySearchCtx::new(wallet.master_seed)?;
        let falcon_pk = ctx.get_falcon_public_key();
        let x25519_pk = ctx.get_x25519_public_key();
        
        println!("\n📋 Public Keys:");
        println!("   Falcon512: {}", hex::encode(&falcon_pk[..32])); // First 32 bytes
        println!("   X25519:    {}", hex::encode(x25519_pk));
    } else {
        let vsk = X25519StaticSecret::from(wallet.view_secret);
        let vpk = X25519PublicKey::from(&vsk);
        
        println!("\n📋 Public Key (X25519):");
        println!("   {}", hex::encode(vpk.as_bytes()));
    }
    
    println!("\n✅ Wallet created successfully!");
    Ok(())
}

fn cmd_info(name: String, wallet_dir: &PathBuf) -> anyhow::Result<()> {
    let wallet = WalletKeys::load(&name, wallet_dir)?;
    
    println!("📊 Wallet Information");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Name:          {}", wallet.name);
    println!("Quantum-safe:  {}", if wallet.quantum_enabled { "✓ Yes" } else { "✗ No" });
    
    if wallet.quantum_enabled {
        let ctx = QuantumKeySearchCtx::new(wallet.master_seed)?;
        let falcon_pk = ctx.get_falcon_public_key();
        let x25519_pk = ctx.get_x25519_public_key();
        
        println!("\n🔑 Public Keys:");
        println!("Falcon512 (897 bytes):");
        println!("  {}", hex::encode(&falcon_pk[..64])); // Show first 64 bytes
        println!("  ...");
        println!("X25519:");
        println!("  {}", hex::encode(x25519_pk));
    } else {
        let vsk = X25519StaticSecret::from(wallet.view_secret);
        let vpk = X25519PublicKey::from(&vsk);
        
        println!("\n🔑 Public Key:");
        println!("X25519:");
        println!("  {}", hex::encode(vpk.as_bytes()));
    }
    
    Ok(())
}

fn cmd_send(to: String, amount: u64, memo: Option<String>, quantum: bool) -> anyhow::Result<()> {
    println!("💸 Building payment hint");
    println!("   To:        {}", &to[..16]);
    println!("   Amount:    {}", amount);
    println!("   Quantum:   {}", if quantum { "✓" } else { "✗" });
    
    // Parse recipient public key
    let to_bytes = hex::decode(&to)?;
    if to_bytes.len() != 32 {
        anyhow::bail!("Invalid recipient public key length");
    }
    
    let mut c_out = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut c_out);
    
    let mut r_blind = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut r_blind);
    
    // Build memo
    let memo_items = if let Some(text) = memo {
        vec![
            keysearch::tlv::Item::Ascii(text),
            keysearch::tlv::Item::ValuePlain(amount),
        ]
    } else {
        vec![keysearch::tlv::Item::ValuePlain(amount)]
    };
    
    let memo_encoded = keysearch::tlv::encode(&memo_items);
    
    // Build hint
    let payload = HintPayloadV1 {
        r_blind,
        value: amount,
        memo: memo_encoded,
    };
    
    let mut to_pk = [0u8; 32];
    to_pk.copy_from_slice(&to_bytes);
    let recipient_pk = X25519PublicKey::from(to_pk);
    
    let enc_hint = keysearch::KeySearchCtx::build_enc_hint_ext(
        &recipient_pk,
        &c_out,
        keysearch::AadMode::COutOnly,
        Some(r_blind),
        keysearch::ValueConceal::Masked(amount),
        &memo_items,
    );
    
    println!("\n📦 Payment Hint Generated:");
    println!("c_out:    {}", hex::encode(c_out));
    println!("hint:     {}", hex::encode(&enc_hint));
    println!("hint_len: {} bytes", enc_hint.len());
    
    println!("\n✅ Send this to the recipient!");
    Ok(())
}

fn cmd_receive(name: String, hint: String, c_out: String, wallet_dir: &PathBuf) -> anyhow::Result<()> {
    println!("📨 Scanning for payment...");
    
    let wallet = WalletKeys::load(&name, wallet_dir)?;
    let hint_bytes = hex::decode(&hint)?;
    let c_out_bytes = hex::decode(&c_out)?;
    
    if c_out_bytes.len() != 32 {
        anyhow::bail!("Invalid c_out length");
    }
    
    let mut c_out_array = [0u8; 32];
    c_out_array.copy_from_slice(&c_out_bytes);
    
    // Try to match hint
    let ctx = keysearch::KeySearchCtx::new(wallet.view_secret);
    
    match ctx.try_match_and_decrypt_ext(&c_out_array, &hint_bytes, keysearch::AadMode::COutOnly) {
        Some((k_search, decoded_opt)) => {
            println!("✅ Payment found!");
            println!("k_search: {}", hex::encode(k_search));
            
            if let Some(decoded) = decoded_opt {
                println!("\n📋 Payment Details:");
                if let Some(value) = decoded.value {
                    println!("   Amount: {}", value);
                }
                println!("   r_blind: {}", hex::encode(decoded.r_blind));
                
                if !decoded.memo_items.is_empty() {
                    println!("   Memo:");
                    for item in decoded.memo_items {
                        match item {
                            keysearch::tlv::Item::Ascii(s) => println!("     Text: {}", s),
                            keysearch::tlv::Item::ValuePlain(v) => println!("     Value: {}", v),
                            _ => {}
                        }
                    }
                }
            }
        }
        None => {
            println!("❌ No matching payment found");
        }
    }
    
    Ok(())
}

fn cmd_export(name: String, format: String, wallet_dir: &PathBuf) -> anyhow::Result<()> {
    let wallet = WalletKeys::load(&name, wallet_dir)?;
    
    println!("📤 Exporting public keys for: {}", name);
    
    if format == "json" {
        if wallet.quantum_enabled {
            let ctx = QuantumKeySearchCtx::new(wallet.master_seed)?;
            let falcon_pk = ctx.get_falcon_public_key();
            let x25519_pk = ctx.get_x25519_public_key();
            
            let json = serde_json::json!({
                "wallet": name,
                "quantum_enabled": true,
                "falcon_pk": hex::encode(falcon_pk),
                "x25519_pk": hex::encode(x25519_pk),
            });
            
            println!("{}", serde_json::to_string_pretty(&json)?);
        } else {
            let vsk = X25519StaticSecret::from(wallet.view_secret);
            let vpk = X25519PublicKey::from(&vsk);
            
            let json = serde_json::json!({
                "wallet": name,
                "quantum_enabled": false,
                "x25519_pk": hex::encode(vpk.as_bytes()),
            });
            
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    } else {
        // Hex format
        if wallet.quantum_enabled {
            let ctx = QuantumKeySearchCtx::new(wallet.master_seed)?;
            let x25519_pk = ctx.get_x25519_public_key();
            println!("{}", hex::encode(x25519_pk));
        } else {
            let vsk = X25519StaticSecret::from(wallet.view_secret);
            let vpk = X25519PublicKey::from(&vsk);
            println!("{}", hex::encode(vpk.as_bytes()));
        }
    }
    
    Ok(())
}

/* ===== MAIN ===== */

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    
    // Determine wallet directory
    let wallet_dir = if let Some(dir) = cli.wallet_dir {
        dir
    } else {
        dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
            .join(".qfw")
    };
    
    match cli.command {
        Commands::Keygen { name, quantum } => cmd_keygen(name, quantum, &wallet_dir),
        Commands::Info { name } => cmd_info(name, &wallet_dir),
        Commands::Send { to, amount, memo, quantum } => cmd_send(to, amount, memo, quantum),
        Commands::Receive { name, hint, c_out } => cmd_receive(name, hint, c_out, &wallet_dir),
        Commands::Export { name, format } => cmd_export(name, format, &wallet_dir),
    }
}
