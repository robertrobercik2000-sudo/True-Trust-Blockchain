# TRUE-TRUST-PROTOCOL 🔐

A post-quantum secure blockchain implementation with deterministic consensus and advanced cryptographic primitives.

## 🌟 Features

- **Post-Quantum Cryptography**: Falcon-512 signatures and Kyber-768 key encapsulation
- **Deterministic Consensus**: PRO consensus with trust, quality, and stake components
- **Advanced Cryptography**: KMAC256, RandomX PoW, STARK/Winterfell ZK proofs
- **Golden Trio Model**: Sophisticated validator quality assessment system
- **Secure P2P**: Post-quantum secure communication channels

## 🏗️ Architecture

### Core Components

1. **Consensus Layer** (`consensus_pro.rs`)
   - Deterministic PQ-only consensus mechanism
   - Weight formula: `W = T × Q × S` (trust × quality × stake)
   - Fixed-point Q32.32 arithmetic (no floating-point in consensus path)

2. **Cryptographic Primitives**
   - **Falcon-512**: Digital signatures (128-bit post-quantum security)
   - **Kyber-768**: Key encapsulation mechanism
   - **KMAC256**: Key derivation and message authentication
   - **RandomX**: ASIC-resistant proof of work
   - **STARK**: Zero-knowledge proofs via Winterfell

3. **Golden Trio Quality System** (`golden_trio.rs`)
   - 6 trust components: Block production, Proof generation, Uptime, Stake lock, Fees, Community
   - Final weight: `W_final = T^1.0 × (1+R)^0.5 × S^0.8`

## 🚀 Quick Start

### Prerequisites

- Rust 1.70+ (2021 edition)
- C compiler (for PQClean FFI)
- POSIX shell (for setup scripts)

### Building

```bash
# Clone the repository
git clone https://github.com/yourusername/TRUE-TRUST-PROTOCOL.git
cd TRUE-TRUST-PROTOCOL

# Build all components
cargo build --release

# Run tests
cargo test

# Build with all features
cargo build --release --all-features
```

### Running a Node

```bash
# Start a validator node
cargo run --release --bin tt_node -- validator \
  --data-dir ./node-data \
  --p2p-port 9090 \
  --rpc-port 8080

# Run wallet CLI
cargo run --release --bin tt_node -- wallet \
  --keystore ./keystore
```

## 📁 Project Structure

```
TRUE-TRUST-PROTOCOL/
├── Cargo.toml              # Workspace configuration
├── tt_node/                # Main blockchain node
│   ├── src/
│   │   ├── main.rs         # Node entry point
│   │   ├── consensus_pro.rs # PRO consensus implementation
│   │   ├── golden_trio.rs  # Validator quality system
│   │   ├── crypto/         # Cryptographic modules
│   │   │   ├── kmac.rs     # KMAC256 implementation
│   │   │   └── kmac_drbg.rs # Deterministic RNG
│   │   ├── falcon_sigs.rs  # Falcon-512 signatures
│   │   ├── kyber_kem.rs    # Kyber-768 KEM
│   │   └── p2p/            # P2P networking
│   ├── examples/           # Usage examples
│   └── tests/              # Integration tests
└── falcon_seeded/          # Deterministic Falcon-512
    ├── src/
    ├── c/                  # PQClean C code
    └── pqclean/            # Post-quantum algorithms

```

## 🔒 Security Features

### Post-Quantum Security
- **Falcon-512**: NIST-approved signature scheme
- **Kyber-768**: NIST-approved KEM
- **KMAC256**: Quantum-resistant hash function
- **RandomX**: CPU-friendly, ASIC-resistant PoW

### Consensus Security
- Deterministic leader selection
- Byzantine fault tolerance
- Slashing for misbehavior
- Trust-based reputation system

## 📊 Consensus Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `TRUST_DECAY` | Trust score decay factor | 0.99 |
| `MIN_STAKE` | Minimum validator stake | 1000 TT |
| `BLOCK_TIME` | Target block time | 6 seconds |
| `EPOCH_LENGTH` | Blocks per epoch | 100 |

## 🛠️ Development

### Running Tests

```bash
# Unit tests
cargo test

# Integration tests
cargo test --test '*'

# Specific module tests
cargo test consensus_pro

# Benchmarks
cargo bench
```

### Examples

```bash
# Consensus rewards simulation
cargo run --example consensus_rewards_test

# End-to-end demo
cargo run --example e2e_demo

# Test all features
cargo run --example test_all_features
```

## 📝 License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.

## 🤝 Contributing

Contributions are welcome! Please read our [Contributing Guide](CONTRIBUTING.md) for details on our code of conduct and the process for submitting pull requests.

## 📚 Documentation

- [Consensus Design](tt_node/CONSENSUS_DESIGN.md)
- [Node Architecture](tt_node/NODE_ARCHITECTURE.md)
- [Trust System Explained](tt_node/TRUST_EXPLAINED.md)

## 🔗 Resources

- [Falcon Specification](https://falcon-sign.info/)
- [Kyber Specification](https://pq-crystals.org/kyber/)
- [NIST PQC Standards](https://csrc.nist.gov/projects/post-quantum-cryptography)

## ⚠️ Disclaimer

This is experimental software. Use at your own risk. Not audited for production use.
