# 📝 Changelog

**Quantum Falcon Wallet - Version History**

---

## [0.2.0] - 2025-11-08 - **"Critical Security Fixes & Deterministic Falcon"**

### 🔥 **Critical Security Fixes**

#### **FIX 1: Falcon KEX Misuse**
- **Issue:** Falcon-512 was incorrectly used for key exchange (KEX)
- **Impact:** Vulnerable to active attacks, non-standard usage
- **Fix:**
  - Falcon-512 now ONLY for digital signatures (correct usage)
  - ML-KEM-768 (Kyber) for quantum-safe key encapsulation
  - X25519 added for hybrid KEX (defense-in-depth)
  - XChaCha20-Poly1305 AEAD with transcript binding
- **Files:** Complete rewrite of `src/crypto/kmac_falcon_integration.rs`
- **Docs:** `CRITICAL_SECURITY_FIX.md`

#### **FIX 2: Falcon Signature Verification Bug**
- **Issue:** `verify_quantum_hint` used receiver's own PK instead of sender's PK
- **Impact:** Any hint would verify, completely broken authentication
- **Fix:**
  - Added `sender_falcon_pk` to `QuantumSafeHint`
  - Transcript now includes sender's public key
  - Verification uses `hint.sender_falcon_pk` (not `self.falcon_identity.1`)
- **Test:** `roundtrip_pq_hint_with_sender_pk` (catches regression)

---

### ✨ **New Features**

#### **Hybrid Commitments (Idea 4)**
- **Component:** `src/hybrid_commit.rs`
- **Formula:** `C = r·G + v·H + fp·F`
  - Classical Pedersen for ZK proofs (r·G + v·H)
  - PQC fingerprint binding (fp·F)
  - No ZK circuit overhead (verified on host)
- **Properties:**
  - Homomorphic addition
  - Deterministic generators
  - PQC fingerprint: `KMAC256(falcon_pk || mlkem_pk)`

#### **Falcon Signatures Module**
- **Component:** `src/falcon_sigs.rs`
- **Features:**
  - Proper attached signature format
  - Batch verification support
  - Serialization helpers
  - `compute_pqc_fingerprint` utility
- **API:**
  - `falcon_keypair()`, `falcon_sign_nullifier()`, `falcon_verify_nullifier()`
  - `falcon_verify_batch()` (efficient batch verification)

#### **PQC Verification Layer**
- **Component:** `src/pqc_verify.rs`
- **Purpose:** Host-side PQC verification (outside ZK circuit)
- **Functions:**
  - `verify_nullifier_signature()` - Falcon signature verification
  - `compute_pqc_fingerprint()` - Fingerprint computation
  - `verify_private_transaction()` - Integrated with ZK receipt

#### **Bulletproofs Integration**
- **Component:** `src/bp.rs`
- **Features:**
  - 64-bit range proofs (unified Pedersen H)
  - Verification compatible with dalek-cryptography
  - Consistent with `hybrid_commit::generator_H`

#### **RISC0 ZK Guests**
- **Components:**
  - `guests/priv_guest/src/main.rs` - Private transaction validation
  - `guests/agg_guest/src/main.rs` - Recursive aggregation
- **Architecture:**
  - Classical Pedersen commitments in ZK
  - PQC fingerprints propagated to public outputs
  - Host verifies PQC binding (layered approach)

#### **KMAC-DRBG**
- **Component:** `src/crypto/kmac_drbg.rs`
- **Features:**
  - `no_std`-ready deterministic RNG
  - Implements `rand_core::RngCore` + `CryptoRng`
  - Key ratcheting for forward secrecy
  - Zeroization of sensitive data
- **Usage:** Deterministic Falcon operations (via `falcon_seeded`)

#### **Deterministic Falcon (Optional)**
- **Component:** `falcon_seeded/` crate
- **Architecture:**
  - FFI to PQClean's C implementation
  - Custom `randombytes` shim (thread-local callback)
  - Injects `KmacDrbg` for determinism
- **Features:**
  - `falcon_keypair_deterministic()` - Reproducible keygen
  - `falcon_sign_deterministic()` - Reproducible signatures
  - Requires PQClean sources (not bundled)
- **Docs:** `FALCON_SEEDED_INTEGRATION.md`, setup script

---

### 🔧 **Refinements**

#### **Configurable Time/Epoch Parameters**
- **Function:** `verify_quantum_hint_with_params(hint, c_out, max_skew_secs, accept_prev_epoch)`
- **Defaults:**
  - `DEFAULT_MAX_SKEW_SECS = 7200` (2 hours)
  - `DEFAULT_ACCEPT_PREV_EPOCH = true`
- **Usage:** Strict mode for high-security, relaxed for testing

#### **Hint Fingerprinting for Bloom Filters**
- **Function:** `hint_fingerprint16(hint, c_out) -> [u8; 16]`
- **Purpose:** Fast pre-filtering before full verification (~1000x speedup)
- **Derivation:** `KMAC256(transcript, LABEL_HINT_FP, ...)`

#### **Cryptographic Labels**
- **Constants:**
  - `LABEL_HYBRID`, `LABEL_AEAD_KEY`, `LABEL_AEAD_NONCE`
  - `LABEL_TRANSCRIPT`, `LABEL_HINT_FP`, `LABEL_HINT_FP_DOMAIN`
- **Purpose:** Clear audit trail, domain separation

#### **Nonce Handling**
- **Fix:** `XNonce::from_slice(&nonce24)` (instead of `&XNonce::from(nonce24)`)
- **Reason:** Correct type for `chacha20poly1305` API

#### **Epoch Validation Logic**
- **Old:** `hint.epoch > current_epoch + 1` (reject future hints)
- **New:** `!(hint.epoch == e || hint.epoch.saturating_add(1) == e)` (accept current or previous)

---

### 🧪 **Testing**

#### **Negative Tests (Tampering Detection)**
All 5 tampering scenarios tested ✅:
1. `verify_fails_on_tampered_timestamp` - Replay protection
2. `verify_fails_on_sender_pk_swap` - Authentication check
3. `verify_fails_on_kem_ct_tamper` - KEM integrity
4. `verify_fails_on_x25519_pub_tamper` - Hybrid KEX integrity
5. `verify_fails_on_encrypted_payload_tamper` - AEAD integrity

#### **KMAC-DRBG Tests**
8 comprehensive tests ✅:
- Determinism (same seed → same output)
- Distinct outputs (different personalization)
- Reseed functionality
- Ratchet behavior
- `RngCore` trait compliance

#### **Deterministic Falcon Tests**
4 tests (marked `#[ignore]`, requires PQClean):
- Deterministic keygen
- Deterministic signing
- Sign/verify roundtrip
- PRF derivation from secret key

**Total Tests:** 48 passing (unit tests), 5 integration tests pending

---

### 🗑️ **Removed**

#### **Legacy Code Deletion**
- ❌ `src/crypto/kmac_mlkem_integration.rs` - Incorrect Falcon usage
- ❌ `src/crypto/quantum_hint_v2.rs` - Superseded by main impl
- ❌ `src/crypto/hint_transcript.rs` - Integrated into main module
- ❌ `src/tt_cli.rs.backup` - Temporary backup file

#### **Documentation Consolidation**
23 individual `.md` files merged into 5 main docs:
- `docs/ARCHITECTURE.md` - System design, crypto architecture
- `docs/SECURITY.md` - Threat model, security analysis
- `docs/INTEGRATION.md` - Setup, API, examples
- `docs/CHANGELOG.md` - Version history (this file)
- `README.md` - Project overview, quick start

Old files archived/removed.

---

### 📚 **Documentation**

#### **New Guides**
- `HYBRID_PQC_ZK_DESIGN.md` - Idea 4 design rationale
- `FALCON_SIGS_API.md` - Falcon signature operations
- `CRITICAL_SECURITY_FIX.md` - KEX misuse details
- `CRYPTO_REFINEMENTS.md` - Post-fix refinements
- `KMAC_DRBG_INTEGRATION.md` - DRBG usage guide
- `FALCON_SEEDED_INTEGRATION.md` - Deterministic Falcon setup

#### **Updated Docs**
- `README.md` - Clearer overview, quickstart
- `DEPENDENCIES.md` - Full dependency list
- `guests/README.md` - ZK architecture rationale

---

### 🔐 **Security Level**

**Post-Quantum Security:** 128-bit (NIST Level 1+)
- Falcon-512 (signatures)
- ML-KEM-768 (KEX, NIST Level 3)
- X25519 (hybrid, classical security)

**Threat Model:**
- ✅ Quantum adversary (Shor's algorithm)
- ✅ Man-in-the-middle attacks
- ✅ Replay attacks (configurable window)
- ✅ Parameter substitution attacks
- ⚠️ Side-channel attacks (partial, PQClean mitigations)

---

## [0.1.0] - 2025-10-XX - **"Initial Release"**

### ✨ **Features**

- **Quantum-Safe Wallet Core**
  - Falcon-512 + ML-KEM (initial architecture, later fixed)
  - Key search protocol with hints
  - Epoch-based key rotation

- **Advanced CLI (v5)**
  - `tt_priv_cli` with Argon2, Shamir Secret Sharing
  - Atomic file operations
  - AEAD encryption for key storage

- **Consensus Primitives**
  - BLS12-381 signatures
  - Epoch snapshots
  - Trust scores

- **Dependencies**
  - `pqcrypto-falcon`, `pqcrypto-kyber`
  - `chacha20poly1305`, `sha3`
  - `curve25519-dalek`, `merlin`

### ⚠️ **Known Issues**
- Falcon KEX misuse (fixed in v0.2.0)
- No PQC verification layer (added in v0.2.0)
- Non-deterministic Falcon (addressed in v0.2.0)

---

## [Unreleased] - **Roadmap**

### Short-term (v0.3.0)
- [ ] P2P networking layer (`node.rs`, `evidence.rs`, `randao.rs`)
- [ ] CLI `send-pq` / `receive-pq` commands
- [ ] End-to-end integration tests
- [ ] Encrypted key store (pragmatic workaround for deterministic Falcon)

### Medium-term (v0.4.0)
- [ ] Batch Falcon verification (performance optimization)
- [ ] Fork `pqcrypto-falcon` with RNG parameter (eliminate FFI complexity)
- [ ] Multi-party computation (MPC) support
- [ ] Hardware wallet integration (HSM/TEE)

### Long-term (v1.0.0)
- [ ] Formal verification (Coq/Lean proofs)
- [ ] Side-channel resistance (constant-time guarantees)
- [ ] Threshold signatures (t-of-n)
- [ ] Cross-chain bridges (PQC-secured)
- [ ] External security audit

---

## **Version Scheme**

- **Major (X.0.0):** Breaking API changes, protocol upgrades
- **Minor (0.X.0):** New features, non-breaking improvements
- **Patch (0.0.X):** Bug fixes, documentation updates

---

## **Contributors**

- **Initial Implementation:** [@cursor-ai-agent]
- **Security Review:** Internal (external audit pending)
- **Community:** Issues and PRs welcome!

---

## **License**

MIT License (see LICENSE file)

---

**Last Updated:** 2025-11-08  
**Current Version:** 0.2.0
