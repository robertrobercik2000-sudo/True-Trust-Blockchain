# Quick Start Guide

## 🚀 Get Started in 5 Minutes

### 1. Build the Project

```bash
# Clone and enter directory
cd /workspace

# Build wallet CLI (without ZK features)
cargo build --release --no-default-features

# Or build with full features (requires ZK implementation)
cargo build --release
```

### 2. Create Your First Wallet

```bash
# Create a new wallet
./target/release/tt_priv_cli wallet-init \
  --file my_wallet.bin \
  --pepper none

# You'll be prompted for a password (min 12 characters)
# Enter a strong password and confirm it
```

### 3. View Your Address

```bash
./target/release/tt_priv_cli wallet-addr --file my_wallet.bin

# Output:
# address: tt1...
# scan_pk (x25519): abc123...
# spend_pk(ed25519): def456...
```

### 4. Create Backup Shards (2-of-3)

```bash
# Create shards directory
mkdir -p ./backup

# Generate shards
./target/release/tt_priv_cli shards-create \
  --file my_wallet.bin \
  --out-dir ./backup \
  --m 2 \
  --n 3

# You now have 3 shards, any 2 can recover your wallet
# Store them in separate secure locations!
```

### 5. Test Recovery

```bash
# Simulate wallet loss - recover from 2 shards
./target/release/tt_priv_cli shards-recover \
  --input ./backup/ttshard-01-of-03-*.json,./backup/ttshard-02-of-03-*.json \
  --out recovered_wallet.bin \
  --pepper none

# Verify it matches
./target/release/tt_priv_cli wallet-addr --file recovered_wallet.bin
```

---

## 📚 Common Operations

### Change Password

```bash
./target/release/tt_priv_cli wallet-rekey --file my_wallet.bin
```

### Export Keys (DANGEROUS!)

```bash
# Public keys only (safe)
./target/release/tt_priv_cli wallet-export --file my_wallet.bin

# Secret keys (requires --out file)
./target/release/tt_priv_cli wallet-export \
  --file my_wallet.bin \
  --secret \
  --out secrets.json

# ⚠️ WARNING: Keep secrets.json EXTREMELY secure!
```

### Use Stronger Encryption

```bash
# XChaCha20 instead of AES-GCM-SIV
./target/release/tt_priv_cli wallet-init \
  --file secure_wallet.bin \
  --aead x-cha-cha20 \
  --pepper os-local

# With OS-local pepper for maximum security
# Pepper stored at: ~/.config/tt/pepper/<wallet_id>
```

### Create Password-Protected Shards

```bash
./target/release/tt_priv_cli shards-create \
  --file my_wallet.bin \
  --out-dir ./backup \
  --m 3 \
  --n 5 \
  --per-share-pass

# Enter password to encrypt ALL shards
# You'll need this password when recovering
```

---

## 🧪 Test the Consensus Module

```bash
cd pot80_zk_host

# Run all consensus tests
cargo test

# Run specific test
cargo test randao_commit_reveal

# Run with output
cargo test -- --nocapture
```

### Example: Create Epoch Snapshot

```rust
use pot80_zk_host::consensus::*;

// Setup
let mut registry = Registry::default();
let mut trust = TrustState::default();

let params = TrustParams {
    alpha_q: q_from_basis_points(9900),  // 99% retention
    beta_q: q_from_basis_points(100),    // 1% reward
    init_q: q_from_basis_points(1000),   // 10% initial
};

// Register validators
let node_a = [1u8; 32];
let node_b = [2u8; 32];
registry.insert(node_a, 1_000_000, true);
registry.insert(node_b, 2_000_000, true);

// Set initial trust
trust.set(node_a, q_from_basis_points(5000));  // 50%
trust.set(node_b, q_from_basis_points(8000));  // 80%

// Create epoch snapshot
let snapshot = EpochSnapshot::build(1, &registry, &trust, &params, 0);

println!("Epoch 1 snapshot:");
println!("  Root: {:?}", hex::encode(snapshot.weights_root));
println!("  Total weight: {}", snapshot.sum_weights_q);
```

---

## ⚙️ Configuration Options

### Wallet Initialization Flags

| Flag | Values | Default | Description |
|------|--------|---------|-------------|
| `--argon2` | true/false | true | Use Argon2id (slower, more secure) |
| `--aead` | gcm-siv, x-cha-cha20 | gcm-siv | Encryption algorithm |
| `--pepper` | none, os-local | os-local | Use OS-stored pepper |
| `--pad-block` | 256-4096 | 1024 | Padding block size |

### Shamir Backup Constraints

- **M**: 2 ≤ M ≤ 8 (threshold)
- **N**: M ≤ N ≤ 8 (total shards)
- Common choices: 2-of-3, 3-of-5, 4-of-7

---

## 🔒 Security Best Practices

### ✅ DO
- ✅ Use strong passwords (≥12 chars, mixed case, symbols)
- ✅ Store shards in separate physical locations
- ✅ Keep pepper backups (if using os-local)
- ✅ Use `--pepper os-local` for production
- ✅ Test recovery before funding wallet

### ❌ DON'T
- ❌ Store wallet + all shards together
- ❌ Commit wallet files to version control
- ❌ Share secret exports
- ❌ Use weak passwords
- ❌ Skip testing recovery procedure

---

## 🐛 Troubleshooting

### "pepper file size invalid"
```bash
# Remove corrupted pepper
rm ~/.config/tt/pepper/<wallet_id>

# Or on Windows:
# del %APPDATA%\TT\pepper\<wallet_id>
```

### "wallet version unsupported"
- Use matching CLI version
- Don't mix wallet files from different versions

### "shard MAC mismatch"
- Shard file corrupted
- Wrong wallet_id (shards don't match)
- Use different shard copy

### Forgot Password
1. Use Shamir recovery (if you created shards)
2. No password = No recovery (by design)

---

## 📊 Performance Tips

### Speed Up Unlock (Development Only!)
```bash
# Use KMAC instead of Argon2 (MUCH faster, less secure)
./target/release/tt_priv_cli wallet-init \
  --file fast_wallet.bin \
  --argon2 false \
  --pepper none
```

### Reduce Argon2 Memory (Low-RAM Systems)
Edit `src/main.rs`:
```rust
let mem_kib: u32 = 128 * 1024;  // 128 MiB instead of 512 MiB
```

---

## 🎯 Next Steps

1. **Read Full Documentation**
   - `README.md` - Complete feature guide
   - `CONSENSUS_MODULE.md` - Deep dive into PoT
   - `IMPLEMENTATION_SUMMARY.md` - Architecture overview

2. **Explore Source Code**
   - `src/main.rs` - Wallet implementation
   - `pot80_zk_host/src/consensus.rs` - Consensus logic
   - `pot80_zk_host/src/snapshot.rs` - Witness system

3. **Run Tests**
   ```bash
   cargo test --all
   ```

4. **Build Your Application**
   - Use as library: `cargo add tt_priv_cli`
   - Integrate consensus: `use pot80_zk_host::consensus::*`

---

## 💡 Tips

- **Backup Strategy**: Store shards with trusted friends/family
- **Testing**: Always test recovery on testnet first
- **Updates**: Keep CLI version updated
- **Monitoring**: Log wallet operations for audit trail

---

## 🆘 Need Help?

- Check `README.md` for detailed documentation
- Run `--help` on any command: `./tt_priv_cli wallet-init --help`
- Review test cases in source code for examples
- See `IMPLEMENTATION_SUMMARY.md` for architecture

---

**Happy Hacking! 🎉**
