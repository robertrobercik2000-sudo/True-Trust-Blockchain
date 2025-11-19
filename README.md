# TRUE_TRUST Blockchain

**Research prototype of post-quantum blockchain consensus.**

## What Actually Works

- **PQ Wallet CLI v5** (`tt_priv_cli`): Falcon512 + Kyber768, Argon2id KDF, Shamir secret sharing
- **RTT PRO**: Deterministic trust algorithm (Q32.32 fixed-point, EWMA, vouching)
- **Consensus weights**: Integer-based validator selection (no floats on hot path)
- **P2P channel**: XChaCha20-Poly1305 + KMAC-KDF for post-handshake encryption

## What's NOT Production

- STARK proofs: educational implementation (BabyBear), unoptimized
- Winterfell integration: stubbed, requires Rust 1.87+
- RandomX: FFI wrapper exists but not required for CLI
- Full node: basic structure, no network layer

## Build

```bash
cargo build --release --bin tt_priv_cli
```

## Wallet Commands

```bash
tt_priv_cli wallet-init --file wallet.tt
tt_priv_cli wallet-addr --file wallet.tt
tt_priv_cli shards-create --file wallet.tt --out-dir shards/ --m 2 --n 3
```

## Tech Stack

- **Signatures**: Falcon512 (NIST Level 1)
- **KEM**: Kyber768 (NIST Level 3)
- **AEAD**: AES-GCM-SIV / XChaCha20-Poly1305
- **KDF**: Argon2id + KMAC256
- **Addresses**: Bech32m (prefix `ttq`)

## Current State

This is a **research prototype**, not production software.
- No security audit
- No formal verification
- No mainnet
- No optimized ZK proofs
- No tested attack resistance

## Consensus Model

```
Weight = 4·Trust + 2·Quality + 1·Stake

Trust T(v) = S(β₁·H + β₂·V + β₃·W)
  H = historical quality (EWMA)
  V = vouching (web of trust)
  W = current work quality
  S(x) = 3x² − 2x³ (smooth curve)
```

All arithmetic in Q32.32 fixed-point (deterministic across platforms).

## License

MIT (see LICENSE file)

## Disclaimer

**Research code.** Do not use for real value. No guarantees.
