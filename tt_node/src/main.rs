// src/main.rs
#![forbid(unsafe_code)]

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
mod stark_security;
mod range_proof_winterfell;
mod stark_full;       // <-- Winterfell STARK range proofs
mod tx_stark;         // <-- Transactions with STARK proofs
mod crypto;           // <-- tu siedzi kmac.rs i kmac_drbg.rs
mod pqc_verification;

mod p2p; // p2p/mod.rs
mod e2e_demo; // E2E demo: Bob & Alice

fn main() {
    // Run E2E demo
    if let Err(e) = e2e_demo::run_demo() {
        eprintln!("❌ Demo failed: {}", e);
        std::process::exit(1);
    }
}
