# TODO List - Quantum Falcon Wallet

**Last Updated:** 2025-11-08  
**Status:** Core implementation complete, production hardening needed

---

## 🔥 **CRITICAL (Security)**

### 1. ⚠️ **End-to-End Integration Test**
**Priority:** P0  
**Status:** ❌ TODO  
**Description:** Test complete flow: wallet → ZK guest → host verify

```bash
# Required test:
1. Create wallet with PQC keys (tt_priv_cli)
2. Create note with hybrid commitment
3. Spend note in ZK guest (priv_guest)
4. Verify receipt + Falcon signature (pqc_verify)
5. Aggregate multiple TXs (agg_guest)
```

**Estimate:** 2-3 hours  
**Files:** `tests/integration_e2e.rs` (new)

---

### 2. ⚠️ **Security Audit Prep**
**Priority:** P0  
**Status:** ❌ TODO  
**Description:** Prepare codebase for external security audit

**Checklist:**
- [ ] Remove all TODOs and placeholders
- [ ] Add security assumptions to docs
- [ ] Document threat model
- [ ] List known limitations
- [ ] Create security.md

**Estimate:** 1 day  
**Files:** `SECURITY.md`, `THREAT_MODEL.md`

---

### 3. ⚠️ **Formal Verification (Hybrid Commitment)**
**Priority:** P1  
**Status:** ❌ TODO  
**Description:** Prove binding property of C = r·G + v·H + fp·F

**Approach:**
- Use Coq or Lean4
- Prove: fp binding implies PQC key binding
- Verify KMAC collision resistance assumption

**Estimate:** 1-2 weeks (expert needed)  
**Files:** `proofs/hybrid_commit.v` or `.lean`

---

## 🏗️ **IMPLEMENTATION (Production Ready)**

### 4. **Build ZK Guests for RISC0**
**Priority:** P0  
**Status:** ❌ TODO  
**Description:** Compile guests to RISC0 ELF format

```bash
# TODO:
cd guests/priv_guest
cargo build --release --target riscv32im-risc0-zkvm-elf

cd ../agg_guest
cargo build --release --target riscv32im-risc0-zkvm-elf
```

**Blockers:**
- Need RISC0 toolchain installed
- May need to adjust Cargo.toml features

**Estimate:** 30 minutes (if toolchain ready)  
**Files:** `guests/*/target/riscv32im-risc0-zkvm-elf/release/*`

---

### 5. **Host ZK Verifier Integration**
**Priority:** P0  
**Status:** ❌ TODO  
**Description:** Add RISC0 receipt verification to `pqc_verify.rs`

```rust
// TODO in pqc_verify.rs:
use risc0_zkvm::Receipt;

pub fn verify_private_transaction_with_zk(
    receipt: &Receipt,
    signatures: &[NullifierSignature],
    ctx: &mut impl PqcVerificationContext,
) -> Result<()> {
    // 1. Verify ZK receipt
    risc0_zkvm::verify(PRIV_GUEST_ID, receipt)?;
    
    // 2. Decode journal
    let claim: PrivClaim = from_journal(receipt)?;
    
    // 3. Verify PQC signatures
    verify_private_transaction(ctx, &claim, signatures)?;
    
    Ok(())
}
```

**Estimate:** 2 hours  
**Files:** `src/pqc_verify.rs`, `Cargo.toml` (add risc0-zkvm)

---

### 6. **Performance Benchmarks**
**Priority:** P1  
**Status:** ❌ TODO  
**Description:** Benchmark all critical operations

```bash
# TODO:
cargo bench --features tt-full
```

**Metrics Needed:**
- Falcon sign/verify (expect ~10ms / 200μs)
- Hybrid commitment (expect ~1μs)
- ZK guest cycles (expect ~10M)
- E2E TX time (expect ~70ms)

**Estimate:** 1 day  
**Files:** `benches/crypto.rs`, `benches/zk.rs`

---

## 🔧 **ENHANCEMENTS (Nice to Have)**

### 7. **Hardware Acceleration (AVX-512)**
**Priority:** P2  
**Status:** ❌ TODO  
**Description:** Use SIMD for Falcon operations

**Approach:**
- Use `falcon-rust` instead of `pqcrypto-falcon`
- Enable AVX-512 features
- Benchmark improvement (expect 2-3× speedup)

**Estimate:** 3-4 days  
**Files:** `src/falcon_sigs.rs`, `Cargo.toml`

---

### 8. **Key Rotation Mechanism**
**Priority:** P2  
**Status:** ❌ TODO  
**Description:** Safely rotate Falcon keys without losing funds

```rust
// TODO:
pub fn rotate_falcon_key(
    old_pk: &FalconPublicKey,
    new_pk: &FalconPublicKey,
    notes: &[Note],
) -> Result<Vec<Note>> {
    // Re-encrypt notes with new key
    // Maintain nullifier validity
}
```

**Estimate:** 2 days  
**Files:** `src/key_rotation.rs` (new)

---

### 9. **HSM Integration**
**Priority:** P2  
**Status:** ❌ TODO  
**Description:** Support hardware security modules

**Approach:**
- PKCS#11 interface
- AWS CloudHSM support
- YubiHSM support

**Estimate:** 1 week  
**Files:** `src/hsm/` (new)

---

### 10. **CLI Commands for PQC**
**Priority:** P1  
**Status:** ❌ TODO  
**Description:** Add PQC-specific commands to `tt_priv_cli`

```bash
# TODO commands:
tt_priv_cli falcon-keygen           # Generate Falcon keypair
tt_priv_cli falcon-sign <msg>       # Sign message
tt_priv_cli falcon-verify <sig>     # Verify signature
tt_priv_cli hybrid-commit <v> <r>   # Create hybrid commitment
tt_priv_cli pqc-fingerprint <pk>    # Compute fingerprint
```

**Estimate:** 4 hours  
**Files:** `src/tt_priv_cli.rs`

---

## 📚 **DOCUMENTATION**

### 11. **Tutorial / Quickstart**
**Priority:** P1  
**Status:** ❌ TODO  
**Description:** Step-by-step guide for users

**Outline:**
1. Installation
2. Generate PQC wallet
3. Create note with hybrid commitment
4. Spend note (ZK + Falcon signature)
5. Verify transaction

**Estimate:** 4 hours  
**Files:** `TUTORIAL.md`

---

### 12. **Performance Profiling Guide**
**Priority:** P2  
**Status:** ❌ TODO  
**Description:** How to profile and optimize

**Tools:**
- flamegraph
- perf
- RISC0 profiler

**Estimate:** 2 hours  
**Files:** `docs/PROFILING.md`

---

## 🧪 **TESTING**

### 13. **Property-Based Tests**
**Priority:** P1  
**Status:** ❌ TODO  
**Description:** Use `proptest` for fuzzing

```rust
// TODO:
proptest! {
    #[test]
    fn hybrid_commit_binding(v: u64, r: [u8; 32], fp: [u8; 32]) {
        // Test binding property
    }
}
```

**Estimate:** 1 day  
**Files:** `tests/proptests.rs` (new)

---

### 14. **Side-Channel Tests**
**Priority:** P1  
**Status:** ❌ TODO  
**Description:** Test for timing leaks

**Tools:**
- dudect
- ctgrind

**Estimate:** 2 days  
**Files:** `tests/sidechannel.rs` (new)

---

## 🌐 **DEPLOYMENT**

### 15. **Docker Container**
**Priority:** P1  
**Status:** ❌ TODO  
**Description:** Containerize for easy deployment

```dockerfile
# TODO: Dockerfile
FROM rust:1.80
RUN apt-get update && apt-get install -y risc0-zkvm
COPY . /app
RUN cargo build --release
```

**Estimate:** 2 hours  
**Files:** `Dockerfile`, `docker-compose.yml`

---

### 16. **CI/CD Pipeline**
**Priority:** P1  
**Status:** ❌ TODO  
**Description:** Automated testing + deployment

**Required:**
- GitHub Actions workflow
- Automated tests on PR
- Security scans (cargo-audit)
- Benchmark regression checks

**Estimate:** 4 hours  
**Files:** `.github/workflows/ci.yml`

---

## 📊 **SUMMARY**

### **Priority P0 (Critical - Do First)**
1. ❌ End-to-End Integration Test
2. ❌ Security Audit Prep
3. ❌ Build ZK Guests
4. ❌ Host ZK Verifier Integration

**Estimate:** 1-2 days

### **Priority P1 (Important - Do Soon)**
5. ❌ Formal Verification
6. ❌ Performance Benchmarks
7. ❌ CLI Commands for PQC
8. ❌ Tutorial / Quickstart
9. ❌ Property-Based Tests
10. ❌ Docker Container
11. ❌ CI/CD Pipeline

**Estimate:** 1 week

### **Priority P2 (Nice to Have)**
12. ❌ Hardware Acceleration
13. ❌ Key Rotation
14. ❌ HSM Integration
15. ❌ Side-Channel Tests
16. ❌ Performance Profiling Guide

**Estimate:** 2-3 weeks

---

## ✅ **COMPLETED**
- [x] Falcon-512 signature module (`falcon_sigs.rs`)
- [x] Hybrid commitment scheme (`hybrid_commit.rs`)
- [x] Bulletproofs support (`bp.rs`)
- [x] PQC verification layer (`pqc_verify.rs`)
- [x] ZK guests (priv_guest, agg_guest)
- [x] Unit tests (35/35 passed)
- [x] API documentation (FALCON_SIGS_API.md)
- [x] Architecture documentation (HYBRID_PQC_ZK_DESIGN.md)
- [x] Module exports in lib.rs

---

**Next Step:** Start with P0 tasks (End-to-End test + Build guests)
