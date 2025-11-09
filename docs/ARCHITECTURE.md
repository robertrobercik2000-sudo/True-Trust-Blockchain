# 🏗️ Project Architecture

**Quantum Falcon Wallet - Post-Quantum Cryptography + Zero-Knowledge Privacy**

---

## 📐 **System Architecture**

```
┌─────────────────────────────────────────────────────────────┐
│                     APPLICATION LAYER                        │
│  ┌───────────────┐  ┌────────────────┐  ┌───────────────┐  │
│  │   CLI (TTQ)   │  │  P2P Node      │  │   ZK Prover   │  │
│  │   tt_cli.rs   │  │  (planned)     │  │  (RISC0)      │  │
│  └───────┬───────┘  └────────┬───────┘  └───────┬───────┘  │
└──────────┼──────────────────┼──────────────────┼───────────┘
           │                  │                  │
┌──────────▼──────────────────▼──────────────────▼───────────┐
│                     CORE LIBRARY                             │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  Quantum-Safe Hints (src/crypto/)                    │  │
│  │  - kmac_falcon_integration.rs  (main impl)           │  │
│  │  - kmac_drbg.rs               (deterministic RNG)    │  │
│  │  - seeded.rs (optional)       (Falcon FFI)           │  │
│  └──────────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  Hybrid Commitments (src/hybrid_commit.rs)           │  │
│  │  - C = r·G + v·H + fp·F                              │  │
│  │  - PQC fingerprint binding                           │  │
│  └──────────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  Falcon Signatures (src/falcon_sigs.rs)              │  │
│  │  - Proper attached signatures                        │  │
│  │  - Batch verification                                │  │
│  └──────────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  PQC Verification (src/pqc_verify.rs)                │  │
│  │  - Host-side nullifier verification                  │  │
│  └──────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
           │                  │                  │
┌──────────▼──────────────────▼──────────────────▼───────────┐
│                    CRYPTOGRAPHIC PRIMITIVES                  │
│  ┌────────────┐  ┌───────────┐  ┌──────────┐  ┌─────────┐ │
│  │ Falcon-512 │  │ ML-KEM    │  │ X25519   │  │ KMAC256 │ │
│  │ (Sig only) │  │ (KEX)     │  │ (Hybrid) │  │ (KDF)   │ │
│  └────────────┘  └───────────┘  └──────────┘  └─────────┘ │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  XChaCha20-Poly1305 (AEAD with transcript binding)    │ │
│  └────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
           │
┌──────────▼──────────────────────────────────────────────────┐
│              ZERO-KNOWLEDGE LAYER (RISC0)                    │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  priv_guest (guests/priv_guest/src/main.rs)          │  │
│  │  - Classical Pedersen commitments                    │  │
│  │  - PQC fingerprints propagated to host               │  │
│  └──────────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  agg_guest (guests/agg_guest/src/main.rs)            │  │
│  │  - Recursive aggregation of receipts                 │  │
│  └──────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

---

## 🔐 **Cryptographic Architecture**

### Layer 1: Post-Quantum Cryptography

| Component | Role | Algorithm |
|-----------|------|-----------|
| **Digital Signatures** | Sender authentication | Falcon-512 (NIST PQC finalist) |
| **Key Encapsulation** | Quantum-safe KEX | ML-KEM-768 (Kyber) |
| **Hybrid KEX** | Defense-in-depth | X25519 ECDH |
| **Key Derivation** | Domain separation | KMAC256 (cSHAKE256) |
| **AEAD** | Authenticated encryption | XChaCha20-Poly1305 |

**Security Level:** 128-bit post-quantum (NIST Level 1+)

### Layer 2: Zero-Knowledge Proofs

| Component | Type | Purpose |
|-----------|------|---------|
| **Pedersen Commitments** | Classical (Curve25519) | Transaction privacy in ZK |
| **Bulletproofs** | Range proofs (64-bit) | Value hiding |
| **RISC0 zkVM** | Recursive SNARKs | Transaction validation |
| **PQC Fingerprints** | Host-side verification | Bridge ZK ↔ PQC |

**Trust Model:** Layered (classical ZK + PQC verification)

### Layer 3: Hybrid Commitments (Idea 4)

```
C_hybrid = r·G + v·H + fp·F

where:
  r   = blinding factor
  G   = Ristretto basepoint
  v   = value
  H   = Pedersen H generator (deterministic)
  fp  = KMAC256(falcon_pk || mlkem_pk)
  F   = PQC generator (deterministic)
```

**Properties:**
- ✅ Classical ZK proofs work (r·G + v·H is standard Pedersen)
- ✅ PQC binding via fingerprint (fp·F)
- ✅ Homomorphic addition preserved
- ✅ No ZK circuit overhead (PQC verified on host)

---

## 📁 **Project Structure**

```
quantum_falcon_wallet/
├── src/
│   ├── lib.rs                      # Main library entry
│   ├── crypto/
│   │   ├── mod.rs                  # Crypto module exports
│   │   ├── kmac.rs                 # KMAC256 primitives
│   │   ├── kmac_drbg.rs            # Deterministic RNG
│   │   ├── kmac_falcon_integration.rs  # Main quantum-safe impl
│   │   └── seeded.rs (optional)    # Deterministic Falcon FFI
│   ├── hybrid_commit.rs            # 3-generator commitments
│   ├── bp.rs                       # Bulletproofs verification
│   ├── falcon_sigs.rs              # Falcon signature ops
│   ├── pqc_verify.rs               # Host-side PQC verification
│   ├── keysearch.rs                # Key search protocol
│   ├── consensus.rs                # Consensus primitives
│   ├── snapshot.rs                 # Epoch snapshots
│   ├── tt_cli.rs                   # Advanced CLI
│   └── tt_priv_cli.rs              # Privacy-focused CLI (v5)
├── guests/
│   ├── priv_guest/                 # Private transaction ZK guest
│   └── agg_guest/                  # Aggregation ZK guest
├── falcon_seeded/                  # Deterministic Falcon (optional)
│   ├── c/                          # FFI shim to PQClean
│   └── src/lib.rs                  # Rust wrapper
├── docs/                           # Documentation
├── tests/                          # Integration tests
└── Cargo.toml                      # Dependencies
```

---

## 🔄 **Data Flow**

### Transaction Flow (Privacy Mode)

```
1. SENDER
   ├─ Generate ephemeral keys (X25519, ML-KEM)
   ├─ Construct hybrid commitment: C = r·G + v·H + fp·F
   ├─ Build quantum-safe hint:
   │  ├─ ML-KEM + X25519 hybrid KEX → shared secret
   │  ├─ Falcon signature over transcript
   │  └─ XChaCha20-Poly1305 encryption (AAD = transcript)
   └─ Publish (C, hint, ZK proof)

2. RISC0 ZK GUEST
   ├─ Verify classical Pedersen (r·G + v·H)
   ├─ Verify Bulletproof range proof
   ├─ Check Merkle inclusion
   ├─ Propagate PQC fingerprints to public output
   └─ Generate ZK receipt

3. HOST VERIFIER
   ├─ Verify RISC0 receipt
   ├─ Extract PQC fingerprints from public outputs
   ├─ Verify hybrid commitments (fp·F binding)
   └─ Accept/reject transaction

4. RECIPIENT
   ├─ Scan hints with Bloom filter (hint_fingerprint16)
   ├─ Decrypt matching hints:
   │  ├─ ML-KEM decapsulation
   │  ├─ X25519 ECDH
   │  ├─ Verify Falcon signature (sender authentication)
   │  └─ XChaCha20-Poly1305 decryption
   └─ Recover (r, v) and spend note
```

---

## 🎯 **Design Principles**

### 1. **Layered Security**
- Classical ZK for efficiency
- PQC for long-term security
- No single point of failure

### 2. **No Premature Optimization**
- PQC verification on host (not in ZK circuit)
- Deterministic RNG optional (feature flag)
- Modular design for future upgrades

### 3. **Cryptographic Agility**
- Clear interfaces (`FillBytes` trait for RNG)
- Domain separation via const labels
- Easy to swap algorithms (trait-based)

### 4. **Defense in Depth**
- Hybrid KEX (ML-KEM + X25519)
- Transcript binding (MITM protection)
- Replay protection (timestamp + epoch)
- Ratcheting (forward secrecy)

---

## 🔬 **Testing Strategy**

### Unit Tests
- **Cryptographic primitives** (KMAC, DRBG, commitments)
- **Determinism** (same seed → same output)
- **Negative tests** (tampering detection)

### Integration Tests
- **Full transaction flow** (keygen → send → receive)
- **ZK proof generation** (guest execution)
- **Cross-layer verification** (ZK + PQC)

### Property Tests
- **Homomorphic addition** (commitments)
- **Transcript binding** (all parameters)
- **Nonce uniqueness** (AEAD security)

---

## 📊 **Performance Characteristics**

| Operation | Time | Notes |
|-----------|------|-------|
| **Falcon-512 keygen** | ~5ms | Non-deterministic (OS RNG) |
| **Falcon-512 sign** | ~2ms | Per signature |
| **Falcon-512 verify** | ~0.5ms | Batch verification faster |
| **ML-KEM encaps** | ~0.1ms | Quantum-safe KEX |
| **ML-KEM decaps** | ~0.1ms | Decryption |
| **XChaCha20 encrypt** | ~0.01ms | Per 1KB payload |
| **KMAC-DRBG fill** | ~0.001ms | Per 64 bytes |
| **Hint scan (Bloom)** | ~0.001ms | Per hint (16B fingerprint) |
| **Full hint verify** | ~3ms | ML-KEM + Falcon + AEAD |

**Bottleneck:** Falcon operations (sign/verify)  
**Optimization:** Batch verification, deterministic signing (future)

---

## 🚀 **Future Enhancements**

### Short-term (v0.3.0)
- [ ] P2P networking layer
- [ ] Encrypted key store (pragmatic workaround)
- [ ] End-to-end integration tests

### Medium-term (v0.4.0)
- [ ] Deterministic Falcon (fork `pqcrypto-falcon`)
- [ ] Batch signature verification
- [ ] Multi-party computation (MPC) support

### Long-term (v1.0.0)
- [ ] Hardware wallet integration (HSM/TEE)
- [ ] Threshold signatures (t-of-n)
- [ ] Cross-chain bridges (PQC-secured)

---

## 📚 **Related Documents**

- [SECURITY.md](./SECURITY.md) - Threat model, security properties
- [INTEGRATION.md](./INTEGRATION.md) - Setup, API, examples
- [CHANGELOG.md](./CHANGELOG.md) - History of changes

---

**Last Updated:** 2025-11-08  
**Version:** 0.2.0
