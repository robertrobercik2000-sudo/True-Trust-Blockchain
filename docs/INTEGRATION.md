# 🔧 Integration Guide

**Quantum Falcon Wallet - Setup, API, Examples**

---

## 🚀 **Quick Start**

### Prerequisites

```bash
# Rust toolchain (1.70+)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Dependencies (Ubuntu/Debian)
sudo apt-get update
sudo apt-get install build-essential pkg-config libssl-dev
```

### Build

```bash
git clone <repo-url>
cd quantum_falcon_wallet
cargo build --release
```

### Run Tests

```bash
# Unit tests
cargo test --lib

# Integration tests
cargo test --test '*'

# With deterministic Falcon (requires PQClean)
cargo test --features seeded_falcon -- --ignored
```

---

## 📚 **API Overview**

### Core Types

```rust
use quantum_falcon_wallet::{
    QuantumKeySearchCtx,      // Main context for quantum-safe operations
    QuantumSafeHint,          // Encrypted hint structure
    HintPayloadV1,            // Plaintext hint payload
    DecodedHint,              // Decrypted hint result
};
```

### Basic Usage

```rust
// 1. Initialize contexts
let sender_seed = [0x42u8; 32];
let sender = QuantumKeySearchCtx::new(sender_seed)?;

let recipient_seed = [0x99u8; 32];
let recipient = QuantumKeySearchCtx::new(recipient_seed)?;

// 2. Build quantum-safe hint
let c_out = [0xABu8; 32]; // Output commitment
let payload = HintPayloadV1 {
    r_blind: [0x11u8; 32],
    value: 1000,
    memo: vec![],
};

let hint = sender.build_quantum_hint(
    recipient.mlkem_public_key(),
    &recipient.x25519_public_key(),
    &c_out,
    &payload,
)?;

// 3. Verify and decrypt
let (decoded, verified) = recipient
    .verify_quantum_hint(&hint, &c_out)
    .expect("Verification failed");

assert!(verified);
assert_eq!(decoded.value, Some(1000));
```

---

## 🔐 **Quantum-Safe Hints**

### Building Hints

```rust
impl QuantumKeySearchCtx {
    /// Build quantum-safe encrypted hint
    ///
    /// # Parameters
    /// - `recipient_mlkem_pk`: Recipient's ML-KEM public key
    /// - `recipient_x25519_pk`: Recipient's X25519 public key
    /// - `c_out`: Output commitment (32 bytes)
    /// - `payload`: Hint data (r_blind, value, memo)
    ///
    /// # Returns
    /// - `QuantumSafeHint`: Encrypted hint with Falcon signature
    pub fn build_quantum_hint(
        &self,
        recipient_mlkem_pk: &MlkemPublicKey,
        recipient_x25519_pk: &X25519PublicKey,
        c_out: &[u8; 32],
        payload: &HintPayloadV1,
    ) -> Result<QuantumSafeHint, FalconError>;
}
```

### Hint Structure

```rust
pub struct QuantumSafeHint {
    pub kem_ct: Vec<u8>,              // ML-KEM ciphertext
    pub x25519_eph_pub: [u8; 32],     // X25519 ephemeral public key
    pub falcon_signed_msg: Vec<u8>,   // Falcon signature over transcript
    pub encrypted_payload: Vec<u8>,   // XChaCha20-Poly1305 ciphertext
    pub sender_falcon_pk: Vec<u8>,    // Sender's Falcon public key
    pub timestamp: u64,               // Unix timestamp
    pub epoch: u64,                   // Key rotation epoch
}
```

### Verifying Hints

```rust
impl QuantumKeySearchCtx {
    /// Verify quantum hint with default parameters
    ///
    /// Uses DEFAULT_MAX_SKEW_SECS (7200s) and DEFAULT_ACCEPT_PREV_EPOCH (true)
    pub fn verify_quantum_hint(
        &self,
        hint: &QuantumSafeHint,
        c_out: &[u8; 32],
    ) -> Option<(DecodedHint, bool)>;

    /// Verify with custom parameters
    pub fn verify_quantum_hint_with_params(
        &self,
        hint: &QuantumSafeHint,
        c_out: &[u8; 32],
        max_skew_secs: u64,
        accept_prev_epoch: bool,
    ) -> Option<(DecodedHint, bool)>;
}
```

---

## 🔍 **Bloom Filter Integration**

### Hint Fingerprinting

```rust
use quantum_falcon_wallet::hint_fingerprint16;

// Generate 16-byte fingerprint for Bloom filter
let fp = hint_fingerprint16(&hint, &c_out);

// Add to Bloom filter
bloom_filter.insert(&fp);
```

### Scanning Example

```rust
// Fast pre-filtering before full verification
for block_hint in blockchain.hints() {
    let fp = hint_fingerprint16(&block_hint, &my_c_out);
    
    if bloom_filter.contains(&fp) {
        // Potential match! Try full verification
        if let Some((decoded, _)) = ctx.verify_quantum_hint(&block_hint, &my_c_out) {
            println!("Found my note: value = {:?}", decoded.value);
        }
    }
}
```

**Performance:** ~1000x faster than full verification

---

## 🎯 **Hybrid Commitments**

### Creating Commitments

```rust
use quantum_falcon_wallet::hybrid_commit::{
    hybrid_commit,
    pqc_fingerprint,
};

// PQC fingerprint (binds Falcon + ML-KEM keys)
let fp = pqc_fingerprint(&falcon_pk, &mlkem_pk);

// Hybrid commitment: C = r·G + v·H + fp·F
let r = [0x42u8; 32];  // Blinding factor
let v = 1000u64;       // Value
let C = hybrid_commit(r, v, fp);
```

### Verification

```rust
use quantum_falcon_wallet::hybrid_commit::hybrid_verify;

// Verify commitment opens correctly
let valid = hybrid_verify(&C, r, v, fp);
assert!(valid);
```

---

## 🔑 **Deterministic Falcon (Optional)**

### Setup PQClean

```bash
cd falcon_seeded
./scripts/setup_pqclean.sh
```

### Enable Feature

```toml
[dependencies]
quantum_falcon_wallet = { path = ".", features = ["seeded_falcon"] }
```

### Usage

```rust
#[cfg(feature = "seeded_falcon")]
use quantum_falcon_wallet::crypto::seeded::{
    falcon_keypair_deterministic,
    falcon_sign_deterministic,
    derive_sk_prf,
};

// Deterministic keygen
let master_seed = [0x42u8; 32];
let (pk, sk) = falcon_keypair_deterministic(
    master_seed,
    b"epoch=1/identity"
)?;

// Deterministic signing (coins bound to transcript)
let transcript = b"transaction data";
let sk_prf = derive_sk_prf(&sk);
let coins_seed = kmac256_derive_key(&sk_prf, b"coins", transcript);
let sig = falcon_sign_deterministic(
    &sk,
    transcript,
    coins_seed,
    b"signing.v1"
)?;
```

**Security Note:** ALWAYS bind coins to unique context (transcript, epoch, nonce)

---

## 🔬 **KMAC-DRBG**

### Basic Usage

```rust
use quantum_falcon_wallet::crypto::kmac_drbg::KmacDrbg;
use rand_core::RngCore;

// Create deterministic RNG
let seed = [0x42u8; 32];
let mut drbg = KmacDrbg::new(&seed, b"application-context");

// Generate random bytes
let mut output = [0u8; 64];
drbg.fill_bytes(&mut output);

// Reseed with additional entropy
drbg.reseed(b"fresh-entropy");

// Manual ratchet (forward secrecy)
drbg.ratchet();
```

### Configuration

```rust
// Set ratchet interval (default: 65536 blocks ~4 MB)
drbg.set_ratchet_interval(16384); // Ratchet every 1 MB
```

---

## 📦 **Feature Flags**

| Feature | Description | Default |
|---------|-------------|---------|
| `cli` | CLI tools (`clap`, `dirs`, `serde_json`) | ✅ |
| `tt-full` | Full CLI with advanced features | ❌ |
| `seeded_falcon` | Deterministic Falcon via FFI | ❌ |

### Usage

```bash
# Build with CLI
cargo build --features cli

# Build with all features
cargo build --all-features

# Build library only
cargo build --no-default-features
```

---

## 🧪 **Testing**

### Unit Tests

```bash
# All unit tests
cargo test --lib

# Specific module
cargo test --lib crypto::kmac_drbg

# With output
cargo test --lib -- --nocapture
```

### Integration Tests

```bash
# All integration tests
cargo test --test '*'

# Specific test file
cargo test --test integration_test
```

### Deterministic Falcon Tests (requires PQClean)

```bash
cd falcon_seeded
./scripts/setup_pqclean.sh
cd ..
cargo test --features seeded_falcon -- --ignored
```

---

## 🚢 **Deployment**

### Production Build

```bash
cargo build --release --features tt-full
```

**Binary locations:**
- `target/release/qfw` - Main CLI
- `target/release/ttq` - Quantum wallet CLI
- `target/release/tt_priv_cli` - Privacy-focused CLI (v5)

### Docker (Optional)

```dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --features tt-full

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/ttq /usr/local/bin/
CMD ["ttq", "--help"]
```

---

## 🐛 **Troubleshooting**

### Build Fails: "PQClean sources not found"

```bash
# This is expected if seeded_falcon feature is enabled
cd falcon_seeded
./scripts/setup_pqclean.sh
cd ..
cargo clean
cargo build --features seeded_falcon
```

### Tests Fail: "OS error 24 (Too many open files)"

```bash
# Increase file descriptor limit
ulimit -n 4096
cargo test
```

### Linker Errors

```bash
# Install required system libraries
sudo apt-get install libssl-dev pkg-config
```

---

## 📊 **Performance Tips**

### 1. Batch Verification

```rust
// Instead of verifying hints one-by-one
for hint in hints {
    verify_quantum_hint(&hint, &c_out);  // Slow
}

// Use Bloom filter pre-filtering
for hint in hints {
    let fp = hint_fingerprint16(&hint, &c_out);
    if bloom.contains(&fp) {
        verify_quantum_hint(&hint, &c_out);  // Only if potential match
    }
}
```

**Speedup:** ~1000x for scanning

### 2. Parallel Hint Scanning

```rust
use rayon::prelude::*;

let matches: Vec<_> = hints
    .par_iter()
    .filter_map(|hint| {
        ctx.verify_quantum_hint(hint, &c_out)
    })
    .collect();
```

### 3. Cached Key Derivation

```rust
// Cache expensive operations
let sk_prf = derive_sk_prf(&falcon_sk);  // Once per key

// Reuse for multiple signatures
for tx in transactions {
    let coins = kmac256_derive_key(&sk_prf, b"coins", &tx.transcript);
    let sig = falcon_sign_deterministic(&falcon_sk, &tx.transcript, coins, b"v1")?;
}
```

---

## 🔗 **Integration Examples**

### Blockchain Integration

```rust
// Publishing hints on-chain
struct HintHeader {
    fingerprint: [u8; 16],
    hint_data: QuantumSafeHint,
}

impl HintHeader {
    fn new(hint: QuantumSafeHint, c_out: &[u8; 32]) -> Self {
        Self {
            fingerprint: hint_fingerprint16(&hint, c_out),
            hint_data: hint,
        }
    }
}

// Scanning blockchain
fn scan_block(ctx: &QuantumKeySearchCtx, block: &Block, my_outputs: &[(OutputId, [u8; 32])]) {
    for (output_id, c_out) in my_outputs {
        for hint_header in &block.hints {
            let fp = hint_fingerprint16(&hint_header.hint_data, c_out);
            if fp == hint_header.fingerprint {
                if let Some((decoded, _)) = ctx.verify_quantum_hint(&hint_header.hint_data, c_out) {
                    println!("Found output {:?}: value = {:?}", output_id, decoded.value);
                }
            }
        }
    }
}
```

### CLI Integration

```bash
# Generate keys
ttq keygen --output keys.json

# Send transaction with quantum-safe hint
ttq send --to <recipient-pk> --amount 1000 --hint-output hint.bin

# Receive and decrypt
ttq receive --hint hint.bin --keys keys.json
```

---

## 📚 **API Reference**

**Full documentation:**
```bash
cargo doc --open
```

**Key modules:**
- `quantum_falcon_wallet::crypto` - Cryptographic primitives
- `quantum_falcon_wallet::hybrid_commit` - Hybrid commitments
- `quantum_falcon_wallet::falcon_sigs` - Falcon operations
- `quantum_falcon_wallet::pqc_verify` - PQC verification

---

## 🆘 **Support**

- **Issues:** GitHub Issues
- **Security:** security@[domain] (for vulnerabilities)
- **Docs:** `docs/` directory

---

**Last Updated:** 2025-11-08  
**API Version:** 0.2.0
