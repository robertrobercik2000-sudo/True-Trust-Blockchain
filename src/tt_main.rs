//! TT Quantum Wallet - Main binary

use quantum_falcon_wallet::tt_cli;

fn main() {
    if let Err(e) = tt_cli::run_cli() {
        eprintln!("❌ Error: {}", e);
        std::process::exit(1);
    }
}
