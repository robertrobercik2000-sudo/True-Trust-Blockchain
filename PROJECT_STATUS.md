# Project Status

**Last updated:** 2025-11-18

## What Compiles

✅ **Library** (`cargo build --lib`)  
✅ **Wallet CLI** (`cargo build --bin tt_priv_cli`)  
✅ **Node binary** (`cargo build --bin tt_node`) - basic structure only

## What's Implemented

### Wallet (Production-Grade)
- ✅ Falcon512 + Kyber768 keypairs
- ✅ Argon2id KDF with local pepper
- ✅ AES-GCM-SIV / XChaCha20-Poly1305 encryption
- ✅ Shamir M-of-N secret sharing (on master seed)
- ✅ Bech32m addresses (`ttq` prefix)
- ✅ Atomic file operations (fsync on parent dir)

### Consensus (Research Implementation)
- ✅ RTT PRO: Q32.32 fixed-point trust algorithm
- ✅ EWMA history, vouching, S-curve smoothing
- ✅ Deterministic validator weights (no floats)
- ✅ Deterministic leader selection (SHA3-based)
- ⚠️ No actual blockchain integration
- ⚠️ No network consensus testing

### Cryptography
- ✅ KMAC256 (XOF, KDF, MAC)
- ✅ KMAC-DRBG (deterministic RNG)
- ✅ Falcon deterministic keygen (via `falcon_seeded` FFI, feature-gated)
- ✅ P2P channel crypto (post-handshake only)
- ❌ P2P handshake: incomplete
- ❌ Full P2P network: not implemented

### ZK Proofs
- ⚠️ BabyBear STARK: educational only, not optimized
- ⚠️ Goldilocks STARK: unoptimized, proof size ~100-200 KB
- ⚠️ Winterfell v2: integrated but requires Rust 1.87+
- ❌ No GPU acceleration
- ❌ No batch verification
- ❌ No proof aggregation

### RandomX
- ✅ FFI wrapper for Monero-compatible RandomX
- ⚠️ Feature-gated (`randomx-ffi`), not required for wallet
- ❌ Not integrated into consensus yet

## Performance (Unoptimized)

**STARK proving** (Goldilocks, no SIMD):
- Prove time: ~2-4 seconds (64-bit range proof)
- Verify time: ~200-500 ms
- Proof size: ~100-200 KB

**Wallet operations**:
- Argon2id (512 MiB, 3 passes): ~1-2 seconds
- Falcon512 sign: ~1 ms
- Kyber768 encaps/decaps: <1 ms

## Security Status

❌ **No external security audit**  
❌ **No formal verification**  
❌ **No cryptanalysis**  
❌ **No penetration testing**  
❌ **No side-channel analysis**

## Known Issues

1. Winterfell STARK requires Rust 1.87+ (currently using 1.91.1)
2. Some integration tests outdated (API changes in consensus modules)
3. Node P2P networking incomplete
4. No mempool implementation
5. No actual fork-choice implementation
6. STARK proofs not optimized (CPU-only, no parallelization)

## Dependencies

**Core:**
- `pqcrypto-falcon` 0.3
- `pqcrypto-kyber` 0.8
- `sha3` 0.10
- `aes-gcm-siv` 0.11
- `chacha20poly1305` 0.10
- `argon2` 0.5

**Optional:**
- `winterfell` 0.13.1 (feature: `winterfell_v2`)
- `falcon_seeded` (local crate, feature: `seeded_falcon`)
- RandomX C library (feature: `randomx-ffi`)

## Build Requirements

- Rust 1.91.1+ (for Winterfell integration)
- Cargo
- C compiler (for RandomX FFI, if enabled)
- pkg-config (for RandomX FFI, if enabled)

## What's NOT Done

- ❌ Mainnet
- ❌ Testnet
- ❌ Block production
- ❌ Transaction pool
- ❌ Fork choice
- ❌ State transitions
- ❌ Slashing
- ❌ Rewards distribution
- ❌ P2P gossip protocol
- ❌ Bootstrap/sync protocol
- ❌ Wallet UI/UX
- ❌ RPC API
- ❌ Block explorer
- ❌ Documentation (beyond code comments)

## Roadmap (Realistic)

This is a **research prototype**. Production deployment would require:

1. **6-12 months**: Optimize STARK, implement full consensus
2. **3-6 months**: Security audit (multiple firms)
3. **3-6 months**: Testnet + bug fixes
4. **3-6 months**: Formal verification of critical paths

**Total: 18-30 months of funded development.**

## Use Cases (Current)

✅ Cryptographic research  
✅ Algorithm prototyping  
✅ Educational purposes  

❌ Production deployments  
❌ Real value storage  
❌ Mainnet transactions  

## Disclaimer

**This is research code.** It has not been audited, formally verified, or tested in adversarial environments. Do not use for anything involving real value.
