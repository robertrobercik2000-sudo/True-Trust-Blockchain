//! Binary entry point for tt_priv_cli standalone wallet
//! 
//! This is a complete quantum-safe wallet CLI with:
//! - Falcon512 + ML-KEM (Kyber768) post-quantum cryptography
//! - Argon2id KDF with OS-local pepper
//! - AES-GCM-SIV / XChaCha20-Poly1305 AEAD
//! - Shamir M-of-N secret sharing
//! - Atomic file operations

use anyhow::Result;

fn main() -> Result<()> {
    quantum_falcon_wallet::tt_priv_cli::run_cli()
}
