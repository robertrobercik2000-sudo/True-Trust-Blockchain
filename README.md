# TRUE_TRUST Wallet CLI v4

Secure cryptocurrency wallet CLI with advanced encryption and backup features.

## Features

- **Multiple AEAD modes**: AES-256-GCM-SIV or XChaCha20Poly1305
- **Flexible KDF**: KMAC256 or Argon2id with configurable parameters
- **Pepper-based security**: Optional OS-local pepper storage
- **Shamir Secret Sharing**: M-of-N shard backup system
- **Zero-knowledge scanning**: Bloom filter-based transaction scanning
- **Atomic file operations**: Safe wallet file updates

## Security

- `#![forbid(unsafe_code)]` - No unsafe Rust code
- Zeroize for sensitive data cleanup
- Atomic file writes with fsync
- Password masking with `rpassword`
- MAC verification for shards

## Building

```bash
cargo build --release
```

## Usage

See `cargo run -- --help` for full command documentation.

### Initialize wallet

```bash
cargo run -- wallet-init --file wallet.dat
```

### Show address

```bash
cargo run -- wallet-addr --file wallet.dat
```

### Create Shamir shards (3-of-5)

```bash
cargo run -- shards-create --file wallet.dat --out-dir ./shards --m 3 --n 5
```

### Recover from shards

```bash
cargo run -- shards-recover --input shard1.json --input shard2.json --input shard3.json --out recovered.dat
```

## License

Apache-2.0
