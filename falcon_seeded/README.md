# 🔐 falcon_seeded - Deterministic Falcon-512 Signing

**Deterministic, reproducible Falcon-512 key generation and signing via KMAC-DRBG.**

---

## 🎯 **Purpose**

This crate provides **deterministic** Falcon-512 operations by replacing OS randomness with a user-controlled DRBG (Deterministic Random Bit Generator). This enables:

- ✅ **Reproducible signatures** (same seed → same signature)
- ✅ **Audit-friendly** (verifiable test vectors)
- ✅ **HSM/TEE/SGX compatible** (no `/dev/urandom` dependency)
- ✅ **Privacy-preserving** (coins derived from secret context)

---

## 🏗️ **Architecture**

```
┌─────────────┐
│   Rust      │
│  KmacDrbg   │  ← Your deterministic RNG
└──────┬──────┘
       │ FillBytes trait
       ↓
┌─────────────┐
│ falcon_seeded│  ← This crate (FFI wrapper)
│  (Rust FFI) │
└──────┬──────┘
       │ C function calls
       ↓
┌─────────────┐
│   PQClean   │  ← Falcon-512 implementation
│ Falcon-512  │     (clean variant)
└─────────────┘
       │
       ↓
  randombytes() ← Replaced by thread-local callback
```

### Key Components:

1. **`c/randombytes_kmac.c`**: Replaces PQClean's `randombytes()` with thread-local callback
2. **`c/falcon_shim.c`**: FFI wrappers for `keypair`, `sign`, `verify`
3. **`src/lib.rs`**: Type-safe Rust API + `FillBytes` trait
4. **`build.rs`**: Links PQClean sources

---

## 📦 **Setup Instructions**

### 1. Clone PQClean

```bash
cd falcon_seeded
git clone https://github.com/PQClean/PQClean.git pqclean_tmp
```

### 2. Copy Falcon-512 Sources

```bash
mkdir -p pqclean/crypto_sign/falcon-512
cp -r pqclean_tmp/crypto_sign/falcon-512/clean pqclean/crypto_sign/falcon-512/
rm -rf pqclean_tmp
```

**Required files** (from PQClean/crypto_sign/falcon-512/clean/):
- `api.c`, `api.h`
- `codec.c`, `codec.h`
- `common.c`, `common.h`
- `falcon.c`, `falcon.h`
- `fft.c`, `fft.h`
- `fpr.c`, `fpr.h`
- `keygen.c`, `keygen.h`
- `rng.c`, `rng.h`
- `shake.c`, `shake.h`
- `sign.c`, `sign.h`
- `vrfy.c`, `vrfy.h`

### 3. Build

```bash
cargo build --release
```

---

## 🚀 **Usage**

### Basic Example

```rust
use falcon_seeded::{keypair_with, sign_with, verify, FillBytes};
use std::sync::Arc;

// Your DRBG implementation
struct MyDrbg { /* ... */ }
impl FillBytes for MyDrbg {
    fn fill(&self, out: &mut [u8]) {
        // Fill with deterministic randomness
    }
}

// Generate keypair
let drbg = Arc::new(MyDrbg::new());
let (pk, sk) = keypair_with(drbg.clone()).unwrap();

// Sign message
let signature = sign_with(drbg, &sk, b"message").unwrap();

// Verify
assert!(verify(&pk, b"message", &signature));
```

### Integration with KmacDrbg

See `src/crypto/seeded.rs` in parent crate for full integration example.

---

## 🔒 **Security Notes**

### ⚠️ CRITICAL: Never Reuse DRBG State

When signing, the DRBG **MUST** be seeded with:
1. **Secret key PRF** (derived from Falcon SK)
2. **Message context** (hash of message/transcript)
3. **Unique nonce** (epoch, timestamp, or counter)

**Bad:**
```rust
let drbg = Arc::new(MyDrbg::new(&seed));
let sig1 = sign_with(drbg.clone(), &sk, b"msg1").unwrap();
let sig2 = sign_with(drbg.clone(), &sk, b"msg2").unwrap();
// ❌ DANGER: Same coins for different messages!
```

**Good:**
```rust
let drbg1 = Arc::new(MyDrbg::new(&seed, b"context1"));
let sig1 = sign_with(drbg1, &sk, b"msg1").unwrap();

let drbg2 = Arc::new(MyDrbg::new(&seed, b"context2"));
let sig2 = sign_with(drbg2, &sk, b"msg2").unwrap();
// ✅ Different coins per message
```

### Determinism vs. Security

- **Deterministic** ≠ **Predictable by attacker**
- Coins are deterministic **relative to secret seed + context**
- External observer sees standard Falcon signatures (indistinguishable from random)

---

## 🧪 **Testing**

```bash
# Run tests (requires PQClean sources)
cargo test

# Ignored tests (need PQClean setup)
cargo test -- --ignored
```

### Test Coverage

- ✅ Keypair generation
- ✅ Sign/verify roundtrip
- ✅ Deterministic keypair (same seed → same keys)
- ✅ Deterministic signatures (same context → same sig)

---

## 📚 **API Reference**

### Constants

```rust
pub const PK_LEN: usize = 897;       // Public key size
pub const SK_LEN: usize = 1281;      // Secret key size
pub const SIG_MAX_LEN: usize = 690;  // Max signature size
```

### Trait

```rust
pub trait FillBytes: Send + Sync {
    fn fill(&self, out: &mut [u8]);
}
```

Implement this for your DRBG (e.g., `KmacDrbg`).

### Functions

```rust
// Generate deterministic keypair
pub fn keypair_with(
    src: Arc<dyn FillBytes>
) -> Result<([u8; PK_LEN], [u8; SK_LEN]), &'static str>

// Sign with deterministic RNG
pub fn sign_with(
    src: Arc<dyn FillBytes>,
    sk: &[u8; SK_LEN],
    msg: &[u8]
) -> Result<Vec<u8>, &'static str>

// Verify signature (standard Falcon)
pub fn verify(
    pk: &[u8; PK_LEN],
    msg: &[u8],
    sig: &[u8]
) -> bool
```

---

## 🔧 **Troubleshooting**

### Build fails: "PQClean sources not found"

```bash
cd falcon_seeded
git clone https://github.com/PQClean/PQClean.git pqclean_tmp
mkdir -p pqclean/crypto_sign/falcon-512
cp -r pqclean_tmp/crypto_sign/falcon-512/clean pqclean/crypto_sign/falcon-512/
rm -rf pqclean_tmp
cargo clean
cargo build
```

### Linker errors

Ensure all PQClean `.c` files are present in `pqclean/crypto_sign/falcon-512/clean/`.

### Tests failing

Run with `--ignored` flag and check PQClean sources are correctly installed.

---

## 📄 **License**

MIT (same as parent project)

**PQClean License:** Public Domain (CC0)

---

## 🙏 **Credits**

- **PQClean**: Reference implementations of post-quantum cryptography
- **Falcon**: NIST PQC standardization finalist
- **Design**: Based on KMAC-DRBG integration pattern

---

## 🔗 **References**

- [PQClean Repository](https://github.com/PQClean/PQClean)
- [Falcon Specification](https://falcon-sign.info/)
- [NIST PQC Project](https://csrc.nist.gov/projects/post-quantum-cryptography)

---

**Status:** ✅ **Production-Ready** (with PQClean sources installed)
