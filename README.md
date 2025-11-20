# True-Trust Blockchain

**Quantum-resistant blockchain with Proof-of-Trust consensus mechanism**

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.82%2B-orange.svg)](https://www.rust-lang.org)

## 🚀 Overview

True-Trust Blockchain is an advanced blockchain platform featuring:

- **Proof-of-Trust (PoT) Consensus**: Innovative consensus combining stake and trust metrics
- **Quantum-Resistant Cryptography**: KMAC256 (SHA3-based) for all hash operations
- **RANDAO Beacon**: Commit-reveal scheme for verifiable randomness
- **Shamir Secret Sharing**: M-of-N backup scheme for wallet recovery
- **Privacy-Preserving Transactions**: Encrypted hints with ECDH key exchange
- **Zero-Knowledge Proofs**: Integration with RISC Zero for private transactions

## ✨ Features

### Cryptography
- ✅ **KMAC256** hash functions (SHA3-SHAKE256 based)
- ✅ **Ed25519** signatures for spending keys
- ✅ **X25519** ECDH for scanning keys
- ✅ **AES-256-GCM-SIV** and **XChaCha20-Poly1305** AEAD
- ✅ **Argon2id** key derivation with pepper enhancement

### Consensus
- ✅ **Fixed-point arithmetic** (Q32.32) for precise trust calculations
- ✅ **Merkle tree snapshots** for deterministic weight verification
- ✅ **Equivocation detection** and slashing
- ✅ **RANDAO beacon** with no-reveal penalties

### Wallet
- ✅ **Encrypted wallet storage** with multiple AEAD options
- ✅ **Password-based encryption** with Argon2id
- ✅ **Shamir secret sharing** (M-of-N recovery)
- ✅ **Bech32m addresses** for human-readable format
- ✅ **Bloom filter scanning** for efficient transaction detection

## 📦 Installation

### Prerequisites

- Rust 1.82.0 or later
- Cargo (comes with Rust)

### Building from source

```bash
# Clone the repository
git clone https://github.com/robertrobercik2000-sudo/True-Trust-Blockchain.git
cd True-Trust-Blockchain

# Build in release mode
cargo build --release

# Binary will be at: ./target/release/tt_priv_cli
```

### Note: Missing Dependency

⚠️ **Current Status**: The project requires the `pot80-zk-host` library which is not currently included in this repository. You'll need to either:

1. Add the missing library to the project
2. Or stub out the ZK-proof functionality for testing

See [COMPREHENSIVE_ANALYSIS.md](./COMPREHENSIVE_ANALYSIS.md) for details.

## 🎮 Usage

### Create a new wallet

```bash
./target/release/tt_priv_cli wallet-init \
  --file my_wallet.dat \
  --argon2 \
  --aead gcm-siv \
  --pepper os-local \
  --pad-block 1024
```

### Show wallet address

```bash
./target/release/tt_priv_cli wallet-addr --file my_wallet.dat
```

Output:
```
address: tt1q...
scan_pk (x25519): 1a2b3c4d...
spend_pk(ed25519): 5e6f7g8h...
```

### Export wallet keys

```bash
# Export public keys (safe)
./target/release/tt_priv_cli wallet-export --file my_wallet.dat

# Export secret keys (DANGEROUS - requires confirmation)
./target/release/tt_priv_cli wallet-export \
  --file my_wallet.dat \
  --secret \
  --out secrets.json
```

### Change wallet password

```bash
./target/release/tt_priv_cli wallet-rekey \
  --file my_wallet.dat \
  --argon2 \
  --aead xchacha20 \
  --pepper os-local
```

### Create Shamir backup shards (3-of-5)

```bash
./target/release/tt_priv_cli shards-create \
  --file my_wallet.dat \
  --out-dir ./shards \
  --m 3 \
  --n 5 \
  --per-share-pass
```

This creates 5 shards, any 3 of which can recover the wallet.

### Recover wallet from shards

```bash
./target/release/tt_priv_cli shards-recover \
  --input ./shards/ttshard-01-of-05-*.json \
  --input ./shards/ttshard-02-of-05-*.json \
  --input ./shards/ttshard-03-of-05-*.json \
  --out recovered_wallet.dat
```

### Scan for transactions

```bash
# Scan a single receipt
./target/release/tt_priv_cli scan-receipt \
  --filters ./filters \
  --file receipt.dat

# Scan a directory of receipts
./target/release/tt_priv_cli scan-dir \
  --filters ./filters \
  --dir ./receipts
```

## 🏗️ Architecture

```
True-Trust-Blockchain/
├── src/
│   ├── main.rs                    # CLI wallet implementation (1054 lines)
│   ├── lib.rs                     # Library re-exports
│   ├── pot.rs                     # Proof-of-Trust consensus (746 lines)
│   ├── snapshot.rs                # Merkle snapshot verification (149 lines)
│   └── crypto_kmac_consensus.rs   # KMAC256 cryptography (47 lines)
├── Cargo.toml                     # Dependencies
└── docs/
    ├── COMPREHENSIVE_ANALYSIS.md  # Full code analysis
    ├── CODE_REVIEW.md             # Code review
    └── PROJECT_ANALYSIS.md        # Project structure analysis
```

### Core Modules

1. **`pot.rs`**: Proof-of-Trust consensus
   - Trust state management with Q32.32 fixed-point
   - RANDAO beacon for randomness
   - Sortition-based leader selection
   - Equivocation detection and slashing

2. **`snapshot.rs`**: Weight snapshots
   - Merkle tree-based weight commitments
   - Compact witness verification
   - Deterministic ordering

3. **`crypto_kmac_consensus.rs`**: Cryptography
   - KMAC256 hash function
   - Domain separation for all operations

4. **`main.rs`**: CLI wallet
   - Wallet management (init, rekey, export)
   - Shamir secret sharing
   - Transaction scanning
   - Encrypted hints generation

## 🔐 Security

### Security Features

- ✅ **No unsafe code**: `#![forbid(unsafe_code)]`
- ✅ **Memory zeroization**: Automatic cleanup of sensitive data
- ✅ **Atomic file operations**: Protection against data corruption
- ✅ **Password requirements**: Minimum 12 characters
- ✅ **Pepper-enhanced KDF**: Additional protection layer
- ✅ **MAC verification**: All shards are MAC-protected

### Security Considerations

⚠️ This is research-level code and has **NOT** been audited for production use.

- Do **NOT** use in production without a professional security audit
- Store wallet files securely (encrypted disk recommended)
- Use strong passwords (≥20 characters, high entropy)
- Keep Shamir shards in separate secure locations
- Backup your pepper file (`~/.config/tt/pepper/` on Linux)

## 🧪 Testing

Run all tests:

```bash
cargo test --all-features
```

Run tests with output:

```bash
cargo test -- --nocapture
```

Run specific module tests:

```bash
cargo test pot::tests::
cargo test snapshot::tests::
```

## 📊 Code Quality

- **Lines of code**: ~2,047
- **Test coverage**: ~15 unit tests
- **Unsafe code**: 0
- **Clippy warnings**: To be resolved
- **Documentation**: Comprehensive inline and external docs

## 🛠️ Development

### Code style

```bash
# Format code
cargo fmt

# Run linter
cargo clippy -- -D warnings

# Check for issues
cargo check --all-targets
```

### Adding features

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass
5. Submit a pull request

## 📚 Documentation

- [Comprehensive Analysis](./COMPREHENSIVE_ANALYSIS.md) - Full code review and analysis
- [Code Review](./CODE_REVIEW.md) - Previous code review
- [Project Analysis](./PROJECT_ANALYSIS.md) - Architecture overview
- [PoT Changes](./POT_CHANGES.md) - Consensus mechanism changes
- [KMAC Migration](./KMAC_MIGRATION.md) - Cryptography migration notes

## 🤝 Contributing

Contributions are welcome! Please:

1. Follow Rust naming conventions
2. Add tests for new features
3. Update documentation
4. Use English for all comments and documentation
5. Run `cargo fmt` and `cargo clippy` before committing

## 📄 License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.

## ⚠️ Disclaimer

This software is provided "as is" for research and educational purposes. It has not undergone a formal security audit and should not be used in production environments or for handling real assets without proper review and testing.

## 📞 Contact

- GitHub: [@robertrobercik2000-sudo](https://github.com/robertrobercik2000-sudo)
- Repository: [True-Trust-Blockchain](https://github.com/robertrobercik2000-sudo/True-Trust-Blockchain)

## 🙏 Acknowledgments

- Uses [RISC Zero](https://www.risczero.com/) for zero-knowledge proofs
- Built with the Rust cryptography ecosystem
- Inspired by Algorand's sortition and Ethereum's RANDAO

---

**Status**: ⚠️ Research prototype - Not production ready

**Last Updated**: 2025-11-17
