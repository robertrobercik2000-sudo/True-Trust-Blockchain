# Cryptography

## Post-Quantum

| Component | Algorithm | Security |
|-----------|-----------|----------|
| Signatures | Falcon512 | 128-bit classical, 64-bit quantum |
| KEM | Kyber768 | 192-bit classical, 96-bit quantum |
| Hashing | SHA3-256 | 128-bit classical, 64-bit quantum |

## STARK Proofs

**Goldilocks field:** p = 2^64 - 2^32 + 1

**Status:** Unoptimized research implementation
- Prove time: 2-4 seconds (should be <1s)
- Verify time: 300-700ms (should be <200ms)  
- Proof size: 100-200 KB (should be <100 KB)

**Winterfell:** Integrated but requires Rust 1.87+

## KMAC256

Used for:
- KDF (key derivation)
- MAC (authentication)
- XOF (random bytes)
- DRBG (deterministic RNG)

## Files

- `src/falcon_sigs.rs` - Falcon512 wrapper
- `src/kyber_kem.rs` - Kyber768 wrapper
- `src/stark_goldilocks.rs` - STARK (64-bit field)
- `src/crypto/kmac.rs` - KMAC primitives
- `src/crypto/seeded.rs` - Deterministic Falcon keygen
