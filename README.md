# TRUE_TRUST Wallet CLI v4

A secure cryptocurrency wallet CLI with advanced cryptographic features including AEAD encryption, Argon2 key derivation, and Shamir secret sharing for backup shards.

## 🔐 Security Features

### Multi-Layer Encryption
- **AEAD Ciphers**: AES-256-GCM-SIV (default) or XChaCha20-Poly1305
- **Key Derivation**: Argon2id (512 MiB, 3 iterations) or KMAC256
- **Pepper System**: OS-local pepper storage for additional KDF entropy
- **Zeroization**: Automatic memory clearing for sensitive data
- **Atomic Operations**: Safe file writes with fsync

### Key Management
- **Ed25519** signing keys for spending
- **X25519** keys for scanning/encryption
- **KMAC256** key derivation from master seed
- **Bech32m** address encoding

### Backup & Recovery
- **Shamir Secret Sharing**: M-of-N backup shards (2-8 shards)
- **Optional Shard Encryption**: Password-protect individual shards
- **MAC Verification**: Integrity checking for all shards

## 📦 Installation

### Prerequisites
- Rust 1.70+ (2021 edition)
- Cargo

### Build

```bash
# Clone the repository
git clone <repository-url>
cd tt_priv_cli

# Build release binary
cargo build --release

# The binary will be at target/release/tt_priv_cli
```

### Features

- `zk-support` (default): Enables zero-knowledge proof scanning features
- Build without ZK support: `cargo build --release --no-default-features`

## 🚀 Usage

### Wallet Management

#### Initialize a New Wallet

```bash
# Create wallet with default settings (Argon2, AES-GCM-SIV, OS-local pepper)
tt_priv_cli wallet-init --file my_wallet.bin

# Create wallet with XChaCha20 and no pepper
tt_priv_cli wallet-init --file my_wallet.bin \
  --aead x-cha-cha20 \
  --pepper none

# Create wallet with KMAC KDF instead of Argon2
tt_priv_cli wallet-init --file my_wallet.bin \
  --argon2 false
```

#### View Wallet Address

```bash
# Display bech32 address and public keys
tt_priv_cli wallet-addr --file my_wallet.bin
```

#### Export Keys

```bash
# Export public keys (to stdout)
tt_priv_cli wallet-export --file my_wallet.bin

# Export secret keys (requires --out file)
tt_priv_cli wallet-export --file my_wallet.bin \
  --secret \
  --out secrets.json
```

#### Change Wallet Password

```bash
# Re-encrypt wallet with new password
tt_priv_cli wallet-rekey --file my_wallet.bin

# Change cipher and KDF settings
tt_priv_cli wallet-rekey --file my_wallet.bin \
  --aead x-cha-cha20 \
  --argon2 false
```

### Shamir Backup Shards

#### Create Backup Shards

```bash
# Create 3-of-5 Shamir backup shards
tt_priv_cli shards-create \
  --file my_wallet.bin \
  --out-dir ./backups \
  --m 3 \
  --n 5

# Create password-protected shards
tt_priv_cli shards-create \
  --file my_wallet.bin \
  --out-dir ./backups \
  --m 2 \
  --n 3 \
  --per-share-pass
```

#### Recover from Shards

```bash
# Recover wallet from M shards
tt_priv_cli shards-recover \
  --input shard1.json,shard2.json,shard3.json \
  --out recovered_wallet.bin

# Or use multiple --input flags
tt_priv_cli shards-recover \
  --input shard1.json \
  --input shard2.json \
  --input shard3.json \
  --out recovered_wallet.bin
```

### Zero-Knowledge Features (requires `zk-support` feature)

#### Scan for Receipts

```bash
# Scan single receipt
tt_priv_cli scan-receipt \
  --filters ./bloom_filters \
  --file receipt.bin

# Scan directory of receipts
tt_priv_cli scan-dir \
  --filters ./bloom_filters \
  --dir ./receipts/

# Scan header hints
tt_priv_cli scan-header \
  --filters ./bloom_filters \
  --file header.bin
```

#### Key Search Operations

```bash
# Search with full pairs (c_out + enc_hint)
tt_priv_cli keysearch-pairs \
  --wallet my_wallet.bin \
  --file pairs.jsonl

# Stateless search (c_out + eph_pub + enc_hint32)
tt_priv_cli keysearch-stateless \
  --wallet my_wallet.bin \
  --file stateless.jsonl

# Header-only search
tt_priv_cli keysearch-header \
  --wallet my_wallet.bin \
  --file headers.jsonl
```

#### Build Encrypted Hints

```bash
# Build enc_hint for recipient
tt_priv_cli build-enc-hint \
  --scan-pk <hex-encoded-x25519-pk> \
  --c-out <hex-encoded-commitment> \
  --value 1000 \
  --memo-utf8 "Payment for services" \
  --out hint.bin
```

## 🔒 Security Considerations

### Pepper System
- **OS-local pepper** (default): Stored at `~/.config/tt/pepper/<wallet_id>` (Linux/macOS) or `%APPDATA%\TT\pepper\<wallet_id>` (Windows)
- Automatically generated on first use
- Unix systems: Created with 0600 permissions
- **Backup warning**: Pepper files must be backed up separately!

### Password Requirements
- Minimum 12 characters
- No maximum length
- Used with either Argon2id or KMAC256 KDF

### Argon2 Parameters
- **Memory**: 512 MiB (configurable)
- **Time cost**: 3 iterations
- **Parallelism**: 1 lane
- **Algorithm**: Argon2id (v0x13)

### Shamir Secret Sharing
- Threshold: 2 ≤ M ≤ N ≤ 8
- Uses Galois Field GF(256)
- Each shard can be password-protected
- MAC authenticated with KMAC256

## 📁 File Formats

### Wallet File (.bin)
- Binary format (bincode serialization)
- Contains: Header (metadata) + Encrypted payload
- Maximum size: 1 MiB
- Atomic writes with fsync

### Shard Files (.json)
- JSON format for human readability
- Contains: Header + Encrypted share + MAC
- Backward compatible with bincode parsing

### Export Formats
- **Public keys**: Plain text output
- **Secret keys**: JSON format (requires `--out` file)

## 🏗️ Architecture

### Wallet Format (v4)
```
WalletFile {
  header: {
    version: 4,
    kdf: Argon2id | Kmac256,
    aead: AES-GCM-SIV | XChaCha20,
    nonce: [u8],
    padding_block: u16,
    pepper: OsLocal | None,
    wallet_id: [u8; 16]
  },
  enc: Vec<u8>  // AEAD(padded(master32))
}
```

### Key Derivation
```
master32 (32 bytes)
  ├─> spend_sk = KMAC256(master32, "TT-SPEND.v1", "seed")
  │     └─> spend_pk = Ed25519::from(spend_sk)
  └─> scan_sk = KMAC256(master32, "TT-SCAN.v1", "seed")
        └─> scan_pk = X25519::from(scan_sk)
```

### Address Format
```
bech32m("tt", [version_byte=0x01][scan_pk:32][spend_pk:32])
```

## 🧪 Development

### Run Tests
```bash
cargo test
```

### Run with Debug Output
```bash
RUST_LOG=debug cargo run -- wallet-init --file test.bin
```

### Build Without ZK Support
```bash
cargo build --no-default-features
```

## 📋 Command Reference

| Command | Description |
|---------|-------------|
| `wallet-init` | Create new encrypted wallet |
| `wallet-addr` | Show public address and keys |
| `wallet-export` | Export public or secret keys |
| `wallet-rekey` | Change password/encryption |
| `shards-create` | Create Shamir backup shards |
| `shards-recover` | Recover wallet from shards |
| `filters-info` | Show Bloom filter metadata |
| `scan-receipt` | Scan single ZK receipt |
| `scan-dir` | Scan directory of receipts |
| `scan-header` | Scan header hints |
| `keysearch-pairs` | Search with full pairs |
| `keysearch-stateless` | Stateless key search |
| `keysearch-header` | Header-only search |
| `build-enc-hint` | Build encrypted hint |

## 🐛 Troubleshooting

### "pepper file size invalid"
- Pepper file corrupted. Delete and regenerate:
  - Linux/macOS: `rm ~/.config/tt/pepper/<wallet_id>`
  - Windows: Delete `%APPDATA%\TT\pepper\<wallet_id>`

### "wallet version unsupported"
- Wallet created with different version. Use matching CLI version.

### "shard MAC mismatch"
- Shard file corrupted or tampered. Use different shard copy.

### "password mismatch"
- Incorrect password. Try again or restore from backup shards.

## 📄 License

See LICENSE file for details.

## ⚠️ Disclaimer

This is cryptographic software. Use at your own risk. Always:
- Test thoroughly before using with real funds
- Keep multiple backups in secure locations
- Never share your secret keys or master seed
- Verify source code integrity before building
