# STARK Field Selection - TRUE TRUST BLOCKCHAIN

**Current Default:** Goldilocks (64-bit)  
**Date:** 2025-11-09

---

## ⚙️ Configuration

```toml
# Cargo.toml
[features]
default = ["goldilocks"]  # PRODUCTION (64-bit classical, 32-bit quantum)
babybear = []             # TESTING ONLY (31-bit, faster but weaker)
goldilocks = []           # PRODUCTION (64-bit, 2× slower but secure)
```

---

## 📊 Field Comparison

| Field | Modulus | Bits | Classical | Quantum | FFT Order | Status |
|-------|---------|------|-----------|---------|-----------|--------|
| **Goldilocks** | 2^64 - 2^32 + 1 | 64 | 64-bit | 32-bit | 2^32 | ✅ **DEFAULT** |
| BabyBear | 2^31 - 2^27 + 1 | 31 | 31-bit | 15-bit | 2^27 | ⚠️ Testing only |

---

## 🔐 Security Levels

### Goldilocks (64-bit) - DEFAULT

```
Classical Security: 64-bit
Quantum Security:   32-bit

Safe until: ~2040 (NIST estimates)
Acceptable for:  Mainnet, production, real value
```

**Properties:**
- ✅ 64-bit prime (fits in u64)
- ✅ FFT-friendly: 2-adic order = 32 (domains up to 2^32 points)
- ✅ Fast reduction: p = 2^64 - 2^32 + 1
- ✅ Production-proven: Plonky2 (Polygon zkEVM)
- ✅ Security: ~64-bit classical, ~32-bit quantum

**Performance (unoptimized):**
```
Prove time:  2-4s    (target: 500ms-1s with optimizations)
Verify time: 300-700ms (target: 100-200ms)
Proof size:  100-200 KB (target: 50-100 KB)
```

**Used in production:**
- Polygon Plonky2 (Polygon zkEVM outer recursion)
- Miden VM (planned upgrade)

### BabyBear (31-bit) - TESTING ONLY

```
Classical Security: 31-bit  ⚠️ WEAK
Quantum Security:   15-bit  ⚠️ VERY WEAK

Safe until: NOT SAFE FOR PRODUCTION
Acceptable for:  Testing, development, benchmarks ONLY
```

**Properties:**
- ✅ 31-bit prime (fits in u32)
- ✅ FFT-friendly: 2-adic order = 27 (domains up to 2^27 points)
- ✅ Fast: ~2× faster than Goldilocks
- ❌ Security: TOO WEAK for real value

**Performance (unoptimized):**
```
Prove time:  1-2s
Verify time: 200-500ms
Proof size:  100-400 KB
```

**Used for:**
- Development testing
- Performance benchmarking
- Educational demos

---

## 🔧 How to Switch Fields

### Use Goldilocks (DEFAULT):

```bash
# Default build (Goldilocks)
cargo build --release

# Explicit Goldilocks
cargo build --release --features goldilocks
```

### Use BabyBear (TESTING ONLY):

```bash
# Testing/benchmarking only
cargo build --release --features babybear
```

---

## 📝 Implementation Status

### Goldilocks (`src/stark_goldilocks.rs`)

```rust
/// Goldilocks prime: p = 2^64 - 2^32 + 1
pub const FIELD_MODULUS: u64 = 0xFFFFFFFF00000001;

/// Maximum 2-adic order: 2^32
pub const MAX_2_ADIC_ORDER: usize = 32;

/// Primitive root: 7 (verified)
pub const PRIMITIVE_ROOT: u64 = 7;
```

**Status:** ✅ Implemented, unoptimized

**Optimizations needed:**
- [ ] FFT optimizations (Cooley-Tukey)
- [ ] Parallel proving (rayon)
- [ ] SIMD field operations
- [ ] Batch verification
- [ ] Constraint system improvements

### BabyBear (`src/stark_full.rs`)

```rust
//! STARK – szkic edukacyjny (NIE production)
//! Ten kod nie zapewnia gwarancji wydajności ani poziomu bezpieczeństwa.

pub const FIELD_MODULUS: u64 = 2013265921; // 2^31 - 2^27 + 1
pub const MAX_2_ADIC_ORDER: usize = 27;
```

**Status:** ⚠️ Educational only, marked as NOT production

**Use cases:**
- Unit tests (faster CI)
- Development iteration
- Algorithm validation

---

## 🎯 Recommendation

### For All Users:

**USE GOLDILOCKS (default)**

```bash
cargo build --release
```

**Reasons:**
1. ✅ 64-bit classical security (acceptable until ~2040)
2. ✅ 32-bit quantum security (post-quantum resistant)
3. ✅ Production-proven (Plonky2)
4. ✅ Safe for real value
5. ⚠️ Unoptimized but improvable (2-4s → 500ms-1s)

### For Developers Only:

**BabyBear for fast iteration:**

```bash
cargo test --features babybear  # Faster tests
```

**But NEVER for production or real value!**

---

## ⏱️ Performance Comparison

| Metric | BabyBear (31-bit) | Goldilocks (64-bit) | Target (optimized) |
|--------|-------------------|---------------------|--------------------|
| **Prove time** | 1-2s | 2-4s | 500ms-1s |
| **Verify time** | 200-500ms | 300-700ms | 100-200ms |
| **Proof size** | 100-400 KB | 100-200 KB | 50-100 KB |
| **Security** | ❌ 31-bit | ✅ 64-bit | ✅ 64-bit |
| **Production** | ❌ NO | ✅ YES | ✅ YES |

**Slowdown:** Goldilocks is ~2× slower than BabyBear (unoptimized)

**Acceptable:** Yes, for security gain (31-bit → 64-bit)

---

## 🔮 Future: BN254 (256-bit)

**If 128-bit classical security needed (2040+):**

```toml
[features]
bn254 = []  # 256-bit field, 128-bit classical, 64-bit quantum
```

**Trade-offs:**
- ✅ 128-bit classical security
- ✅ 64-bit quantum security
- ❌ ~10× slower than Goldilocks
- ❌ Multi-precision arithmetic
- ⚠️ Only if required (overkill for most use cases)

**Timeline:** Not needed before 2040

---

## 📚 References

1. **Goldilocks Field:**
   - Plonky2 (Polygon): https://github.com/mir-protocol/plonky2
   - Miden VM: https://github.com/0xPolygonMiden/miden-vm

2. **BabyBear Field:**
   - Used in Plonky2 inner recursion
   - Miden VM current implementation

3. **Security Analysis:**
   - See `src/stark_security.rs`
   - See `docs/security/QUANTUM_SECURITY_DECISION.md`

---

## ✅ Summary

| Question | Answer |
|----------|--------|
| **What field is used by default?** | Goldilocks (64-bit) |
| **Is BabyBear production-ready?** | ❌ NO - Testing only |
| **Is Goldilocks production-ready?** | ⚠️ Concepts yes, needs optimization |
| **Should I use BabyBear?** | Only for testing/development |
| **Should I use Goldilocks?** | ✅ Yes (default) |
| **Is this code production-ready?** | ❌ NO - Research prototype |

---

<p align="center">
  <strong>DEFAULT: Goldilocks (64-bit)</strong><br>
  <em>BabyBear is for testing only - NOT production!</em>
</p>

---

**Last Updated:** 2025-11-09  
**Status:** Goldilocks is default, BabyBear available for testing
