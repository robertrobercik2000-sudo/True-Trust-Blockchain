# Proposed Fixes for True-Trust-Blockchain

**Generated**: 2025-11-17  
**Priority Order**: Critical → High → Medium → Low

---

## 🔴 CRITICAL FIXES (Must fix immediately)

### FIX #1: Add missing `pot80-zk-host` dependency

**Problem**: Project doesn't compile due to missing dependency.

**Solution Options**:

#### Option A: Create stub implementation (Quick fix for testing)

```bash
mkdir -p pot80-zk-host/src
cd pot80-zk-host
```

Create `pot80-zk-host/Cargo.toml`:
```toml
[package]
name = "pot80-zk-host"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1.0"
sha3 = "0.10"
```

Create `pot80-zk-host/src/lib.rs`:
```rust
pub mod crypto_kmac {
    use sha3::{Shake256, digest::{Update, ExtendableOutput, XofReader}};
    
    pub fn kmac256_derive_key(key: &[u8], label: &[u8], context: &[u8]) -> [u8; 32] {
        let mut hasher = Shake256::default();
        Update::update(&mut hasher, b"TT-KDF-");
        Update::update(&mut hasher, label);
        Update::update(&mut hasher, key);
        Update::update(&mut hasher, context);
        let mut out = [0u8; 32];
        XofReader::read(&mut hasher.finalize_xof(), &mut out);
        out
    }
    
    pub fn kmac256_tag(key: &[u8], label: &[u8], data: &[u8]) -> [u8; 32] {
        let mut hasher = Shake256::default();
        Update::update(&mut hasher, b"TT-MAC-");
        Update::update(&mut hasher, label);
        Update::update(&mut hasher, key);
        Update::update(&mut hasher, data);
        let mut out = [0u8; 32];
        XofReader::read(&mut hasher.finalize_xof(), &mut out);
        out
    }
    
    pub fn kmac256_xof(key: &[u8], label: &[u8], context: &[u8], out_len: usize) -> Vec<u8> {
        let mut hasher = Shake256::default();
        Update::update(&mut hasher, b"TT-XOF-");
        Update::update(&mut hasher, label);
        Update::update(&mut hasher, key);
        Update::update(&mut hasher, context);
        let mut out = vec![0u8; out_len];
        XofReader::read(&mut hasher.finalize_xof(), &mut out);
        out
    }
}

pub mod zk {
    use anyhow::Result;
    
    #[derive(Debug, Clone)]
    pub struct PrivClaim {
        pub outputs: Vec<[u8; 32]>,
    }
    
    pub fn verify_priv_receipt(bytes: &[u8]) -> Result<PrivClaim> {
        // Stub implementation
        Ok(PrivClaim { outputs: Vec::new() })
    }
}

pub mod keyindex {
    use std::path::{Path, PathBuf};
    use anyhow::Result;
    
    pub struct BloomFilter {
        pub m_bits: u64,
        pub k_hashes: u32,
    }
    
    impl BloomFilter {
        pub fn contains(&self, _tag: &u16) -> bool {
            false
        }
    }
    
    pub struct KeyIndex {
        pub epoch: u64,
        pub bloom: BloomFilter,
        pub path: PathBuf,
    }
    
    impl KeyIndex {
        pub fn load_latest(dir: &Path) -> Result<Self> {
            Ok(KeyIndex {
                epoch: 0,
                bloom: BloomFilter { m_bits: 1024, k_hashes: 3 },
                path: dir.to_path_buf(),
            })
        }
    }
}

pub mod headers {
    use anyhow::Result;
    
    pub struct HeaderEntry {
        pub filter_tag16: u16,
    }
    
    pub struct HeaderHints {
        pub entries: Vec<HeaderEntry>,
    }
    
    impl HeaderHints {
        pub fn unpack(_bytes: &[u8]) -> Result<Self> {
            Ok(HeaderHints { entries: Vec::new() })
        }
    }
}

pub mod scanner {
    use anyhow::Result;
    use super::{zk::PrivClaim, keyindex::KeyIndex};
    
    pub struct ScanHit {
        pub filter_tag16: u16,
        pub out_idx: usize,
        pub enc_hint32: [u8; 32],
        pub note_commit_point: [u8; 32],
    }
    
    pub fn scan_claim_with_index(_claim: &PrivClaim, _idx: &KeyIndex) -> Result<Vec<ScanHit>> {
        Ok(Vec::new())
    }
}

pub mod keysearch {
    use anyhow::Result;
    
    pub const MAX_ENC_HINT_BYTES: usize = 1024;
    
    pub enum AadMode {
        COutOnly,
        NetIdAndCOut(u32),
    }
    
    pub enum ValueConceal {
        None,
        Plain(u64),
        Masked(u64),
    }
    
    pub struct DecryptedHint {
        pub value: Option<u64>,
        pub memo_items: Vec<tlv::Item>,
        pub r_blind: [u8; 32],
    }
    
    pub struct KeySearchCtx {
        view_key: [u8; 32],
    }
    
    impl KeySearchCtx {
        pub fn new(view_key: [u8; 32]) -> Self {
            Self { view_key }
        }
        
        pub fn try_match_and_decrypt_ext(
            &self,
            _c_out: &[u8; 32],
            _enc: &[u8],
            _aad_mode: AadMode,
        ) -> Option<([u8; 32], Option<DecryptedHint>)> {
            None
        }
        
        pub fn try_match_stateless(
            &self,
            _c_out: &[u8; 32],
            _eph_pub: &[u8; 32],
            _enc_hint32: &[u8; 32],
        ) -> Option<[u8; 32]> {
            None
        }
        
        pub fn header_hit(&self, _eph_pub: &[u8; 32], _tag16: &[u8; 16]) -> bool {
            false
        }
        
        pub fn build_enc_hint_ext(
            _scan_pk: &x25519_dalek::PublicKey,
            _c_out: &[u8; 32],
            _aad_mode: AadMode,
            _r_blind: Option<[u8; 32]>,
            _value: ValueConceal,
            _items: &[tlv::Item],
        ) -> Vec<u8> {
            Vec::new()
        }
    }
    
    pub mod tlv {
        pub enum Item {
            Ascii(String),
            CiphertextToSpend(Vec<u8>),
        }
    }
}
```

Update `Cargo.toml`:
```toml
[dependencies]
pot80-zk-host = { path = "./pot80-zk-host" }
```

**Status**: ✅ Can be implemented in 30 minutes

---

#### Option B: Point to actual repository (Production fix)

If the library exists elsewhere:

```toml
[dependencies]
pot80-zk-host = { git = "https://github.com/user/pot80-zk-host", branch = "main" }
```

**Status**: Depends on library availability

---

## 🟠 HIGH PRIORITY FIXES

### FIX #2: Add comprehensive README.md

**Status**: ✅ IMPLEMENTED (see `/workspace/README.md`)

---

### FIX #3: Translate Polish comments to English

**Files to update**:
- `src/pot.rs` (line 6, and others)
- `src/main.rs` (line 161)

**Example changes**:

```rust
// Before:
// nowa ścieżka: weryfikacja świadka z snapshot.rs (nie rusza starego API)

// After:
// New path: witness verification from snapshot.rs (doesn't touch old API)
```

**Estimated time**: 1 hour

---

### FIX #4: Add GitHub Actions CI/CD

Create `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

jobs:
  test:
    name: Test Suite
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      
      - name: Run tests
        run: cargo test --all-features --verbose
      
      - name: Run doc tests
        run: cargo test --doc

  clippy:
    name: Clippy
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      
      - name: Run clippy
        run: cargo clippy --all-targets --all-features -- -D warnings

  fmt:
    name: Rustfmt
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      
      - name: Check formatting
        run: cargo fmt --all -- --check

  build:
    name: Build Release
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      
      - name: Build release
        run: cargo build --release --verbose
      
      - name: Upload artifact
        uses: actions/upload-artifact@v3
        with:
          name: tt_priv_cli
          path: target/release/tt_priv_cli

  security:
    name: Security Audit
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      
      - name: Install cargo-audit
        run: cargo install cargo-audit
      
      - name: Run security audit
        run: cargo audit
```

**Estimated time**: 30 minutes

---

## 🟡 MEDIUM PRIORITY FIXES

### FIX #5: Add configuration file support

Create `src/config.rs`:

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub argon2: Argon2Config,
    pub wallet: WalletConfig,
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Argon2Config {
    /// Memory cost in KiB (default: 512 * 1024 = 512 MiB)
    pub mem_kib: u32,
    /// Time cost (iterations, default: 3)
    pub time_cost: u32,
    /// Parallelism lanes (default: 1)
    pub lanes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletConfig {
    /// Default padding block size (default: 1024)
    pub pad_block: u16,
    /// Maximum wallet file size (default: 1 MiB)
    pub max_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Minimum password length (default: 12)
    pub min_password_len: usize,
    /// Shamir maximum N (default: 255)
    pub shamir_max_n: u8,
    /// Shamir minimum M (default: 2)
    pub shamir_min_m: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            argon2: Argon2Config {
                mem_kib: 512 * 1024,
                time_cost: 3,
                lanes: 1,
            },
            wallet: WalletConfig {
                pad_block: 1024,
                max_size: 1 << 20,
            },
            security: SecurityConfig {
                min_password_len: 12,
                shamir_max_n: 255,
                shamir_min_m: 2,
            },
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            Ok(toml::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}
```

Add to `Cargo.toml`:
```toml
toml = "0.8"
```

**Estimated time**: 2 hours

---

### FIX #6: Improve error messages in Shamir recovery

Update `shards_recover` function in `main.rs`:

```rust
fn shards_recover(paths: &[PathBuf]) -> Result<[u8;32]> {
    ensure!(!paths.is_empty(), "no shard files provided");
    
    let mut shards: Vec<(ShardHeader, Vec<u8>)> = Vec::new();
    let mut loaded_info: Vec<(String, u8, u8, u8)> = Vec::new();
    
    eprintln!("🔍 Loading shards...");
    
    for (i, p) in paths.iter().enumerate() {
        eprint!("  [{}/{}] {}... ", i+1, paths.len(), p.display());
        
        let bytes = fs::read(p)
            .with_context(|| format!("failed to read shard file"))?;
        
        let sf: ShardFile = serde_json::from_slice(&bytes)
            .or_else(|_| bincode::deserialize(&bytes))
            .with_context(|| format!("failed to parse shard file"))?;
        
        // MAC verify
        let hdr_bytes = bincode::serialize(&sf.hdr)?;
        let mut mac_input = hdr_bytes.clone();
        mac_input.extend(&sf.share_ct);
        let mac_chk = ck::kmac256_tag(
            &shard_mac_key(&sf.hdr.wallet_id, &sf.hdr.salt32),
            b"TT-SHARD.mac",
            &mac_input
        );
        
        ensure!(mac_chk == sf.mac32, "MAC verification failed");
        
        eprintln!("✅ Shard #{} ({}-of-{})", sf.hdr.idx, sf.hdr.m, sf.hdr.n);
        loaded_info.push((
            p.display().to_string(),
            sf.hdr.idx,
            sf.hdr.m,
            sf.hdr.n
        ));
        shards.push((sf.hdr, sf.share_ct));
    }
    
    // Consistency check
    let (wid, m, n) = (shards[0].0.wallet_id, shards[0].0.m, shards[0].0.n);
    
    ensure!(m >= SHAMIR_MIN_M && n >= m && n <= SHAMIR_MAX_N,
        "Invalid Shamir scheme parameters: m={}, n={}", m, n);
    
    for (i, (h, _)) in shards.iter().enumerate() {
        ensure!(
            h.wallet_id == wid && h.m == m && h.n == n,
            "Shard #{} has mismatched parameters (expected {}-of-{}, got {}-of-{})",
            i+1, m, n, h.m, h.n
        );
    }
    
    if shards.len() < m as usize {
        eprintln!("\n❌ Not enough shards!");
        eprintln!("   Required: {} shards", m);
        eprintln!("   Loaded: {} shards", shards.len());
        eprintln!("\n   Loaded shards:");
        for (path, idx, _, _) in &loaded_info {
            eprintln!("     - Shard #{} from {}", idx, path);
        }
        bail!(
            "Insufficient shards: need at least {} for {}-of-{} recovery, got {}",
            m, m, n, shards.len()
        );
    }

    eprintln!("\n✅ All checks passed. Reconstructing secret...");

    // Password unmask if needed
    let mut rec: Vec<(u8, Vec<u8>)> = Vec::new();
    for (h, ct) in shards {
        let pt = if h.has_pw {
            let pw = Zeroizing::new(
                prompt_password(format!("Password for shard #{}: ", h.idx))?
            );
            shard_mask(&ct, pw.as_str(), &h.salt32)
        } else {
            ct
        };
        rec.push((h.idx, pt));
    }

    let sharks = Sharks(m as usize);
    let shares_iter = rec.into_iter().map(|(i, bytes)| Share::new(i, &bytes));
    let secret = sharks.recover(shares_iter)
        .with_context(|| format!(
            "Shamir recovery failed. Ensure you have {} valid shards from the same wallet.",
            m
        ))?;
    
    let mut out = [0u8; 32];
    out.copy_from_slice(&secret);
    
    eprintln!("✅ Secret recovered successfully!");
    Ok(out)
}
```

**Estimated time**: 1 hour

---

### FIX #7: Add progress indicators

Add dependency:
```toml
indicatif = "0.17"
```

Update `derive_kdf_key`:

```rust
use indicatif::{ProgressBar, ProgressStyle};

fn derive_kdf_key(password: &str, hdr: &KdfHeader, pepper: &[u8]) -> [u8; 32] {
    match &hdr.kind {
        KdfKind::Kmac256V1 { salt32 } => {
            let k1 = ck::kmac256_derive_key(password.as_bytes(), b"TT-KDF.v4.kmac.pre", salt32);
            ck::kmac256_derive_key(&k1, b"TT-KDF.v4.kmac.post", pepper)
        }
        KdfKind::Argon2idV1 { mem_kib, time_cost, lanes, salt32 } => {
            let spinner = ProgressBar::new_spinner();
            spinner.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.green} {msg}")
                    .unwrap()
            );
            spinner.set_message(format!(
                "Computing Argon2id ({}MiB, {} iterations)...",
                mem_kib / 1024,
                time_cost
            ));
            spinner.enable_steady_tick(std::time::Duration::from_millis(100));
            
            let params = Params::new(*mem_kib, *time_cost, *lanes, Some(32))
                .expect("argon2 params");
            let a2 = Argon2::new_with_secret(pepper, Algorithm::Argon2id, Version::V0x13, params)
                .expect("argon2 new_with_secret");
            let mut out = [0u8; 32];
            a2.hash_password_into(password.as_bytes(), salt32, &mut out)
                .expect("argon2");
            let result = ck::kmac256_derive_key(&out, b"TT-KDF.v4.post", salt32);
            
            spinner.finish_with_message("✅ Key derived");
            result
        }
    }
}
```

**Estimated time**: 1 hour

---

## 🟢 LOW PRIORITY FIXES / ENHANCEMENTS

### FIX #8: Add benchmarks

Create `benches/crypto_bench.rs`:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tt_priv_cli::crypto_kmac_consensus::kmac256_hash;

fn benchmark_kmac256(c: &mut Criterion) {
    c.bench_function("kmac256_hash", |b| {
        b.iter(|| {
            kmac256_hash(
                black_box(b"TEST"),
                black_box(&[b"input1", b"input2"])
            )
        })
    });
}

criterion_group!(benches, benchmark_kmac256);
criterion_main!(benches);
```

Add to `Cargo.toml`:
```toml
[dev-dependencies]
criterion = "0.5"

[[bench]]
name = "crypto_bench"
harness = false
```

**Estimated time**: 2 hours

---

### FIX #9: Add integration tests

Create `tests/integration_test.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn test_wallet_lifecycle() {
    let tmp = TempDir::new().unwrap();
    let wallet = tmp.path().join("test_wallet.dat");
    
    // Create wallet
    Command::cargo_bin("tt_priv_cli")
        .unwrap()
        .args(&["wallet-init", "--file"])
        .arg(&wallet)
        .write_stdin("test_password_12345\ntest_password_12345\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("created wallet"));
    
    // Show address
    Command::cargo_bin("tt_priv_cli")
        .unwrap()
        .args(&["wallet-addr", "--file"])
        .arg(&wallet)
        .write_stdin("test_password_12345\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("address: tt1"));
}
```

Add dependencies:
```toml
[dev-dependencies]
assert_cmd = "2.0"
predicates = "3.0"
tempfile = "3.8"
```

**Estimated time**: 3 hours

---

### FIX #10: Add SECURITY.md

Create `SECURITY.md`:

```markdown
# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 4.x     | :white_check_mark: |
| < 4.0   | :x:                |

## Reporting a Vulnerability

**DO NOT** report security vulnerabilities through public GitHub issues.

Instead, please report them to:
- Email: security@truetrust.example.com
- PGP Key: [link to PGP key]

You should receive a response within 48 hours.

## Security Considerations

### Wallet Security
- Store wallet files on encrypted storage
- Use strong passwords (≥20 characters recommended)
- Enable OS-local pepper for additional security
- Keep Shamir shards in separate secure locations
- Regularly backup your wallet

### Cryptographic Security
- Uses SHA3-SHAKE256 (KMAC256) for all hash operations
- Ed25519 for signatures (128-bit security)
- X25519 for ECDH (128-bit security)
- Argon2id for password hashing
- AES-256-GCM-SIV or XChaCha20-Poly1305 for encryption

### Known Limitations
- This is research-level code
- Has NOT been professionally audited
- NOT recommended for production use with real assets
- Quantum resistance claims are theoretical

### Best Practices
- Never share your private keys
- Never share your wallet password
- Use different passwords for each wallet
- Regularly update to latest version
- Monitor security advisories

## Disclosure Policy

- Security issues are fixed in private
- Patches are released without detailed vulnerability information
- Full disclosure occurs 90 days after patch release
- Critical vulnerabilities may be disclosed immediately after patch

## Bug Bounty

Currently, we do not offer a bug bounty program.
```

**Estimated time**: 30 minutes

---

## 📊 IMPLEMENTATION PLAN

### Week 1 (Critical + High Priority)
- [ ] Day 1-2: Fix #1 - Add `pot80-zk-host` stub
- [ ] Day 2: Fix #2 - README.md ✅ DONE
- [ ] Day 3: Fix #3 - Translate comments
- [ ] Day 4-5: Fix #4 - Add CI/CD

### Week 2 (Medium Priority)
- [ ] Day 1-2: Fix #5 - Configuration file
- [ ] Day 2-3: Fix #6 - Better error messages
- [ ] Day 4-5: Fix #7 - Progress indicators

### Week 3 (Low Priority + Polish)
- [ ] Day 1-2: Fix #8 - Benchmarks
- [ ] Day 3-4: Fix #9 - Integration tests
- [ ] Day 5: Fix #10 - Security documentation

---

## ✅ COMPLETED

- ✅ **FIX #2**: README.md created
- ✅ **ANALYSIS**: Comprehensive code review completed

---

## 📝 NOTES

- All fixes maintain backward compatibility
- No breaking API changes
- Security-focused improvements
- Better user experience
- Improved developer experience

**Next Steps**: Start with Critical fixes, then move to High Priority.
