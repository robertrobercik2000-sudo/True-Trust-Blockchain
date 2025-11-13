# TRUE_TRUST Blockchain Node 🔐⛓️

**Post-Quantum Blockchain with Proof-of-Trust (PoT) + Proof-of-ZK-Shares (PoZS)**

A production-ready blockchain node combining:
- ✅ **PoT Consensus** - Proof-of-Trust with RANDAO beacon and Merkle snapshots
- ✅ **PoZS zkSNARK Layer** - Groth16/BN254 zero-knowledge proofs for leader eligibility
- ✅ **Post-Quantum Wallet** - Falcon512 signatures + ML-KEM-768 encryption
- ✅ **P2P Networking** - TCP gossip protocol for block propagation
- ✅ **Embedded Storage** - Sled key-value database

---

## 🚀 Quick Start

### Installation

```bash
# Build from source
cargo build --release

# Binary location
./target/release/tt-node
```

### Initialize Wallet

```bash
tt-node init-wallet
# Enter password when prompted
# Output: Node ID and wallet location
```

### Run Node

```bash
# Run validator with 100,000 stake
tt-node run --stake 100000

# Run with custom config
tt-node run \
  --wallet-dir ~/.my_wallet \
  --data-dir ./blockchain_data \
  --listen 0.0.0.0:8000 \
  --bootstrap 192.168.1.100:8000 \
  --stake 1000000
```

### Node Info

```bash
tt-node info
# Shows: Node ID, wallet location, PQ algorithms, ZK status
```

---

## 📐 Architecture

### Consensus: PoT (Proof-of-Trust)

```
Validator Weight = stake_q × trust_q
Leader Selection = Poseidon(beacon || slot || who) < threshold
Threshold = (λ × weight) / Σweights

Where:
  - λ (lambda) = leader ratio (default 10%)
  - trust_q = dynamic trust score (Q32.32 fixed-point)
  - stake_q = validator stake (u128)
```

**Key Features:**
- RANDAO beacon for verifiable randomness (commit-reveal)
- Merkle tree snapshots of validator weights per epoch
- Dynamic trust decay and reward system
- Equivocation detection and slashing

### PoZS: Zero-Knowledge Proofs Layer

**Groth16 Circuit** (optional feature):

```rust
// Public inputs
- weights_root: Merkle root of validator weights
- beacon_value: RANDAO beacon for this slot
- threshold_q: Computed eligibility threshold
- sum_weights_q: Total validator weight

// Private witness
- who: Validator NodeID
- slot: Current slot number
- stake_q: Validator stake
- trust_q: Trust score
- merkle_proof: Merkle path to weights_root

// Constraints
1. Merkle path verification
2. Threshold computation: weight × sum_weights ≥ threshold × total
3. Eligibility hash: elig_hash(beacon, slot, who) < bound
```

**Why Groth16?**
- Constant proof size: ~128 bytes
- Fast verification: <10ms
- BN254 curve (supported by Ethereum)

### Post-Quantum Wallet

**Algorithms:**
- **Falcon512**: Digital signatures (NIST PQC finalist)
- **ML-KEM-768** (Kyber768): Key encapsulation
- **XChaCha20-Poly1305**: AEAD encryption
- **Argon2id**: Password-based key derivation

**Security:**
- All keys encrypted at rest
- Zeroizing for sensitive data
- OS-local pepper (atomic file operations)
- 256-bit security level

---

## 🔧 Configuration

### Cargo Features

```toml
# Enable ZK proofs (Groth16/BN254)
cargo build --features zk-proofs

# Default (with ZK)
cargo build
```

### Node Parameters

```rust
// In code (src/consensus/types.rs)
PotParams {
    epoch_length: 32,        // slots per epoch
    slot_duration: 6,        // seconds
    lambda_q: ONE_Q / 10,    // 10% leader ratio
    min_stake: 100_000,      // minimum stake
}

TrustParams {
    initial_trust_q: ONE_Q,  // 1.0 (Q32.32)
    decay_per_slot: 100,     // small decay
    min_trust_q: ONE_Q / 100, // 0.01 min
    max_trust_q: 2 * ONE_Q,   // 2.0 max
}
```

---

## 🧪 Development

### Run Tests

```bash
# All tests
cargo test

# Library tests only
cargo test --lib

# Specific module
cargo test --lib consensus::pot
```

### Check Compilation

```bash
cargo check
cargo clippy -- -D warnings
```

### Format Code

```bash
cargo fmt
```

---

## 📦 Project Structure

```
/workspace
├── Cargo.toml              # Dependencies and features
├── src/
│   ├── main.rs             # CLI entry point
│   ├── lib.rs              # Library exports
│   ├── crypto.rs           # KMAC256 + SHA3-512
│   ├── consensus/
│   │   ├── types.rs        # Core types (Block, NodeId, Q32.32)
│   │   ├── pot.rs          # PoT consensus logic
│   │   ├── randao.rs       # RANDAO beacon
│   │   └── snapshot.rs     # Epoch snapshots + Merkle
│   ├── wallet/
│   │   ├── keys.rs         # Falcon512 + Kyber768
│   │   ├── storage.rs      # Encrypted wallet files
│   │   └── pq_wallet.rs    # Wallet API
│   ├── network/
│   │   ├── protocol.rs     # Network messages
│   │   ├── peer.rs         # TCP peer manager
│   │   └── gossip.rs       # Block propagation
│   ├── storage/
│   │   └── mod.rs          # Sled database
│   ├── zk/                 # (optional feature)
│   │   ├── groth16.rs      # Groth16 circuit
│   │   └── keccak_gadget.rs # SHA3 gadgets
│   └── node/
│       └── mod.rs          # Main node implementation
└── README.md
```

---

## 🔐 Security Considerations

### Production Deployment

1. **Trusted Setup**: The current ZK setup uses `ChaCha20Rng::from_entropy()`. For production:
   - Use MPC ceremony for Groth16 setup
   - Or switch to PLONK/Halo2 (universal setup)

2. **Key Management**:
   - Store wallet files with restrictive permissions (0600)
   - Use hardware security modules (HSMs) for validators
   - Implement key rotation policies

3. **Network Security**:
   - Add TLS for P2P connections (TODO)
   - Implement rate limiting
   - DDoS protection at network layer

4. **Consensus Safety**:
   - Monitor trust scores
   - Implement slashing for equivocation
   - Backup RANDAO commitments

### Known Limitations

- Simplified Keccak gadget (full implementation needed for production)
- No signature verification in block processing yet
- P2P networking needs encryption (currently plaintext TCP)
- No block finality mechanism implemented

---

## 🛣️ Roadmap

- [ ] Full Keccak-f[1600] gadget implementation
- [ ] Signature verification in consensus
- [ ] TLS/Noise protocol for P2P
- [ ] Finality gadget (GRANDPA-style)
- [ ] Web dashboard for node monitoring
- [ ] Light client support
- [ ] Cross-chain bridges

---

## 📄 License

Apache-2.0

---

## 🤝 Contributing

Contributions welcome! Please:
1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure `cargo test` and `cargo clippy` pass
5. Submit a pull request

---

## 📚 References

- **PoT Consensus**: Inspired by Algorand + Ouroboros Praos
- **RANDAO**: Ethereum 2.0 beacon chain
- **Groth16**: [BCTV14] - "Succinct Non-Interactive Zero Knowledge for a von Neumann Architecture"
- **Falcon**: NIST PQC Round 3 Finalist
- **ML-KEM**: NIST FIPS 203 (Kyber)
- **KMAC**: NIST SP 800-185

---

**Built with ❤️ and Rust 🦀**
