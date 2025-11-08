# Dependencies Note

## External Host Module

This project depends on `pot80_zk_host` which is an optional external module for zero-knowledge proof functionality.

### Without ZK Support (Default Build)

The project can be built without the ZK module using the fallback implementation:

```bash
cargo build --no-default-features
```

This will use the SHAKE256-based fallback for KMAC operations and disable all ZK-related commands.

### With ZK Support

To enable full ZK functionality, you need to:

1. Obtain or implement the `pot80_zk_host` crate with the following modules:
   - `crypto_kmac` - KMAC256 implementations
   - `zk` - Zero-knowledge proof verification
   - `keyindex` - Bloom filter key indexing
   - `headers` - Header hint structures
   - `scanner` - Receipt scanning functionality
   - `keysearch` - Key search contexts and operations

2. Place it in the project root as `pot80_zk_host/` or adjust the path in `Cargo.toml`

3. Build with ZK support:
```bash
cargo build --release
```

### Required Traits/Functions

The `pot80_zk_host` module should expose:

#### `crypto_kmac` module:
- `fn kmac256_derive_key(key: &[u8], label: &[u8], context: &[u8]) -> [u8; 32]`
- `fn kmac256_xof(key: &[u8], label: &[u8], context: &[u8], len: usize) -> Vec<u8>`
- `fn kmac256_tag(key: &[u8], label: &[u8], msg: &[u8]) -> [u8; 32]`

#### `keyindex` module:
- `struct KeyIndex` with `load_latest(dir: &Path) -> Result<KeyIndex>`

#### `scanner` module:
- `struct ScanHit` with fields: `filter_tag16`, `out_idx`, `enc_hint32`, `note_commit_point`
- `fn scan_claim_with_index(claim: &Claim, index: &KeyIndex) -> Result<Vec<ScanHit>>`

#### `keysearch` module:
- `struct KeySearchCtx` with methods for key search operations
- Various enums and helper functions for transaction decryption

#### `zk` module:
- `fn verify_priv_receipt(bytes: &[u8]) -> Result<Claim>`

#### `headers` module:
- `struct HeaderHints` with `unpack(&[u8]) -> Result<HeaderHints>`

## Current Status

The project is currently configured to:
- ✅ Build successfully without ZK support
- ⚠️ Require `pot80_zk_host` for full functionality
- ✅ Use fallback crypto implementations when ZK is disabled

## Testing Without ZK

You can test basic wallet operations without the ZK module:

```bash
# Build without ZK
cargo build --no-default-features

# Test wallet creation
./target/debug/tt_priv_cli wallet-init --file test.bin --pepper none

# Test address display
./target/debug/tt_priv_cli wallet-addr --file test.bin

# Test Shamir shards
./target/debug/tt_priv_cli shards-create --file test.bin --out-dir ./shards --m 2 --n 3
```
