//! TT Private CLI - Full featured quantum wallet
//! Main binary entry point

#![forbid(unsafe_code)]

use quantum_falcon_wallet::tt_priv_cli;

fn main() {
    if let Err(e) = tt_priv_cli::run_cli() {
        eprintln!("❌ Error: {:#}", e);
        std::process::exit(1);
    }
}
