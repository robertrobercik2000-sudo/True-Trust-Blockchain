# 🔐 TRUE TRUST BLOCKCHAIN

**Post-Quantum Blockchain with Proof-of-Trust Consensus**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.82%2B-orange.svg)](https://www.rust-lang.org/)
[![Security](https://img.shields.io/badge/Quantum%20Security-64--bit-green.svg)](docs/QUANTUM_SECURITY_SUMMARY.md)
[![Status](https://img.shields.io/badge/Status-Q1%202025%20Complete-success.svg)](NLNET_DOCUMENTATION_SUMMARY.md)

---

## 📖 Language / Język

- **[Polski (Polish)](README_PL.md)** - Pełna dokumentacja w języku polskim
- **[English](README_EN.md)** - Full documentation in English

---

## 🎯 Project Overview / Przegląd Projektu

**TRUE TRUST** is a next-generation blockchain combining:

**TRUE TRUST** to blockchain nowej generacji łączący:

- ✅ **100% Post-Quantum Cryptography** (NIST-approved: Falcon512, Kyber768)
- ✅ **Proof-of-Trust (PoT) Consensus** - Revolutionary trust-based consensus
- ✅ **STARK Zero-Knowledge Proofs** - Transparent, quantum-resistant ZK
- ✅ **RandomX Proof-of-Work** - ASIC-resistant, CPU-fair mining
- ✅ **Privacy-Preserving Transactions** - STARK range proofs, Kyber encryption

---

## 🚀 Quick Start / Szybki Start

### Prerequisites / Wymagania

```bash
# Rust 1.82+
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# RandomX library (required for full consensus)
sudo apt install git cmake build-essential
git clone https://github.com/tevador/RandomX
cd RandomX && mkdir build && cd build
cmake .. && make && sudo make install
```

### Build / Kompilacja

```bash
# Clone repository / Sklonuj repozytorium
git clone https://github.com/robertrobercik2000-sudo/True-Trust-Blockchain
cd True-Trust-Blockchain

# Build wallet CLI / Zbuduj portfel CLI
cargo build --release

# Build blockchain node / Zbuduj węzeł blockchain
cargo build --release --bin tt_node

# Run tests / Uruchom testy
cargo test --features goldilocks
```

### Usage / Użycie

```bash
# Create new wallet / Stwórz nowy portfel
./target/release/tt_priv_cli wallet init

# Start blockchain node / Uruchom węzeł blockchain
./target/release/tt_node --port 9333 --data-dir ./data
```

---

## 🏗️ Architecture / Architektura

```
TRUE TRUST Blockchain
│
├─ Consensus Layer (Warstwa Konsensusu)
│  ├─ Proof-of-Trust (PoT) - 2/3 trust + 1/3 stake
│  ├─ RandomX PoW - CPU-fair mining
│  ├─ Recursive Trust Tree (RTT) - Q32.32 fixed-point
│  └─ Deterministic Leader Selection
│
├─ Cryptography Layer (Warstwa Kryptograficzna)
│  ├─ Signatures: Falcon512 (NIST PQC)
│  ├─ Key Exchange: Kyber768 (NIST PQC)
│  ├─ Hashing: SHA3-256, KMAC256
│  └─ AEAD: XChaCha20-Poly1305
│
├─ Zero-Knowledge Layer (Warstwa ZK)
│  ├─ STARK Range Proofs (Goldilocks field)
│  ├─ FRI Protocol (80 queries, 16× blowup)
│  └─ Commitment Binding (SHA3-based)
│
├─ Privacy Layer (Warstwa Prywatności)
│  ├─ Encrypted TX Values (Kyber + XChaCha20)
│  ├─ Stealth Addresses (Bloom filters)
│  └─ ZK Trust Proofs (reputation privacy)
│
└─ Network Layer (Warstwa Sieciowa)
   ├─ PQ-Secure P2P (Falcon + Kyber handshake)
   ├─ Encrypted Channels (XChaCha20-Poly1305)
   └─ Replay Protection (transcript hashing)
```

**See full architecture:** [ARCHITECTURE.md](ARCHITECTURE.md)

**Zobacz pełną architekturę:** [ARCHITECTURE.md](ARCHITECTURE.md)

---

## 🔒 Security / Bezpieczeństwo

### Quantum Security Levels / Poziomy Bezpieczeństwa Kwantowego

| Component | Classical | Quantum | Status |
|-----------|-----------|---------|--------|
| **Signatures** | 256-bit | 128-bit | ✅ Falcon512 (NIST) |
| **Key Exchange** | 256-bit | 128-bit | ✅ Kyber768 (NIST) |
| **Range Proofs** | 64-bit | 32-bit | ✅ STARK/Goldilocks |
| **Hashing** | 128-bit | 64-bit | ✅ SHA3-256 |
| **Overall** | **64-bit** | **32-bit** | ✅ **Production** |

**Security Policy:** [SECURITY.md](SECURITY.md)  
**Quantum Analysis:** [docs/QUANTUM_SECURITY_SUMMARY.md](docs/QUANTUM_SECURITY_SUMMARY.md)

---

## 📊 Key Features / Kluczowe Funkcje

### 1. Proof-of-Trust (PoT) Consensus

Revolutionary consensus combining trust, stake, and proof-of-work:

Rewolucyjny konsensus łączący zaufanie, stake i proof-of-work:

```rust
Weight = (2/3) × Trust + (1/3) × Stake
Trust = RTT_Algorithm(participation, quality, vouching)
Leader = Deterministic_Selection(Weight, RandomX_PoW)
```

**Features:**
- No lottery (deterministic leader selection)
- CPU-only proofs (ASIC-resistant)
- Trust decay for inactive validators
- Slashing for misbehavior

### 2. Post-Quantum Cryptography

100% quantum-resistant using NIST-approved algorithms:

100% odporność kwantowa używając algorytmów zatwierdzonych przez NIST:

- **Falcon512** - Lattice-based signatures (5KB, 2ms)
- **Kyber768** - Module-LWE key exchange (2KB, 1ms)
- **STARK** - Transparent ZK proofs (50KB, 500ms)

### 3. Privacy-Preserving Transactions

Private by default with STARK range proofs:

Prywatność domyślnie z dowodami zakresów STARK:

- Encrypted transaction values (Kyber768)
- STARK range proofs (0-2^64 without revealing value)
- Stealth addresses (Bloom filter optimization)
- ZK trust proofs (reputation privacy)

### 4. STARK Zero-Knowledge Proofs

Transparent, quantum-resistant ZK:

Transparentne, kwantowo-odporne ZK:

- **Goldilocks Prime Field** (2^64 - 2^32 + 1)
- **FRI Protocol** (80 queries, 160-bit soundness)
- **Commitment Binding** (prevents proof reuse)
- **Fast proving** (~500ms on CPU)

---

## 📚 Documentation / Dokumentacja

### Core Documentation / Główna Dokumentacja

- [**README_PL.md**](README_PL.md) - Pełna polska dokumentacja
- [**README_EN.md**](README_EN.md) - Full English documentation
- [**ARCHITECTURE.md**](ARCHITECTURE.md) - System architecture
- [**SECURITY.md**](SECURITY.md) - Security policy & vulnerability reporting

### Technical Documentation / Dokumentacja Techniczna

- [**Quantum Security Summary**](docs/QUANTUM_SECURITY_SUMMARY.md) - Complete quantum security analysis
- [**PoT Consensus**](docs/GOLDEN_TRIO_CONSENSUS.md) - Proof-of-Trust detailed specification
- [**Mining Flow**](docs/MINING_FLOW.md) - Step-by-step mining & rewards
- [**RandomX Integration**](docs/MONERO_RANDOMX_INTEGRATION.md) - CPU-fair PoW implementation
- [**STARK Migration**](docs/BULLETPROOFS_TO_STARK_MIGRATION.md) - ECC to STARK migration guide

### Developer Guides / Przewodniki Deweloperskie

- [**Installation Guide**](docs/INSTALL.md) - Detailed installation instructions
- [**API Reference**](docs/API.md) - Complete API documentation
- [**Contributing Guide**](CONTRIBUTING.md) - How to contribute
- [**Code of Conduct**](CODE_OF_CONDUCT.md) - Community guidelines

---

## 🛠️ Development / Rozwój

### Project Structure / Struktura Projektu

```
true-trust-blockchain/
├── src/
│   ├── main.rs              # Wallet CLI entry point
│   ├── lib.rs               # Library exports
│   ├── pot.rs               # Proof-of-Trust core
│   ├── pot_node.rs          # PoT validator node
│   ├── rtt_trust_pro.rs     # Recursive Trust Tree (Q32.32)
│   ├── pow_randomx_monero.rs # RandomX PoW (Monero-compatible)
│   ├── stark_full.rs        # BabyBear STARK (31-bit, testnet)
│   ├── stark_goldilocks.rs  # Goldilocks STARK (64-bit, mainnet)
│   ├── stark_security.rs    # Security parameter analysis
│   ├── tx_stark.rs          # STARK transactions
│   ├── falcon_sigs.rs       # Falcon512 signatures
│   ├── kyber_kem.rs         # Kyber768 KEM
│   ├── p2p_secure.rs        # PQ-secure P2P transport
│   ├── node_v2_p2p.rs       # Blockchain node with P2P
│   └── ...                  # Other modules
├── docs/                    # Detailed documentation
├── tests/                   # Integration tests
├── benches/                 # Performance benchmarks
├── Cargo.toml               # Rust dependencies
└── build.rs                 # Build script (RandomX linking)
```

### Feature Flags / Flagi Funkcji

```toml
[features]
default = ["goldilocks"]     # Production: 64-bit STARK
babybear = []                # Testnet: 31-bit STARK (fast)
goldilocks = []              # Mainnet: 64-bit STARK (secure)
zk-proofs = [...]            # Enable Groth16/BN254 (optional)
```

---

## 🧪 Testing / Testowanie

```bash
# Run all tests / Uruchom wszystkie testy
cargo test --all-features

# Run security tests / Testy bezpieczeństwa
cargo test --test security --features goldilocks

# Run consensus tests / Testy konsensusu
cargo test pot:: --features goldilocks

# Benchmarks / Benchmarki
cargo bench --features goldilocks
```

---

## 📈 Performance / Wydajność

| Operation | BabyBear (31-bit) | Goldilocks (64-bit) | BN254 (254-bit) |
|-----------|-------------------|---------------------|-----------------|
| STARK Prove | ~250ms | ~500ms | ~5000ms |
| STARK Verify | ~50ms | ~100ms | ~1000ms |
| Proof Size | ~25 KB | ~50 KB | ~200 KB |
| Falcon Sign | ~2ms | ~2ms | ~2ms |
| Kyber KEM | ~1ms | ~1ms | ~1ms |

**Hardware:** Intel i7-10700K @ 3.8GHz, 16GB RAM

---

## 🌍 Community / Społeczność

- **Website:** https://truetrust.blockchain (coming soon)
- **GitHub:** https://github.com/robertrobercik2000-sudo/True-Trust-Blockchain
- **Discord:** https://discord.gg/truetrust (coming soon)
- **Forum:** https://forum.truetrust.blockchain (coming soon)

---

## 🤝 Contributing / Współpraca

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

Zapraszamy do współpracy! Zobacz [CONTRIBUTING.md](CONTRIBUTING.md) dla wytycznych.

### How to Contribute / Jak Pomóc

1. Fork the repository / Zrób fork repozytorium
2. Create feature branch / Stwórz branch z funkcją
3. Write tests / Napisz testy
4. Submit pull request / Wyślij pull request

---

## 📜 License / Licencja

This project is licensed under the **MIT License** - see [LICENSE](LICENSE) file.

Ten projekt jest na licencji **MIT** - zobacz plik [LICENSE](LICENSE).

---

## 🙏 Acknowledgments / Podziękowania

**Note:** This project is being prepared for NLnet Foundation grant application.

**Uwaga:** Ten projekt jest przygotowywany do aplikacji o grant od NLnet Foundation.

### Technical Inspirations / Inspiracje Techniczne

- **NIST** - Post-Quantum Cryptography standards
- **Monero** - RandomX algorithm inspiration
- **StarkWare** - STARK protocol research
- **Plonky2** - Goldilocks field implementation

---

## 📞 Contact / Kontakt

- **Email:** contact@truetrust.blockchain
- **Security Issues:** security@truetrust.blockchain
- **GitHub Issues:** https://github.com/robertrobercik2000-sudo/True-Trust-Blockchain/issues

---

## 🗺️ Roadmap / Plan Rozwoju

### Q1 2025
- ✅ Core consensus implementation (PoT + RandomX)
- ✅ Post-quantum cryptography (Falcon + Kyber)
- ✅ STARK ZK proofs (BabyBear + Goldilocks)
- ✅ Security analysis & documentation

### Q2 2025
- 🔄 Testnet launch
- 🔄 Network layer optimization
- 🔄 Wallet GUI
- 🔄 Block explorer

### Q3 2025
- 📅 Mainnet preparation
- 📅 Third-party security audit
- 📅 BN254 field implementation (optional)
- 📅 Mobile wallet

### Q4 2025
- 📅 Mainnet launch
- 📅 DApp framework
- 📅 Cross-chain bridges
- 📅 Governance system

---

<p align="center">
  <strong>Built with ❤️ for a quantum-safe future</strong><br>
  <strong>Zbudowane z ❤️ dla kwantowo-bezpiecznej przyszłości</strong>
</p>

<p align="center">
  <a href="https://nlnet.nl/">
    <img src="https://nlnet.nl/logo/banner.svg" alt="NLnet Foundation" width="200"/>
  </a>
</p>
