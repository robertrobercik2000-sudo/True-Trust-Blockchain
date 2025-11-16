# 🏗️ TRUE TRUST BLOCKCHAIN - Architecture / Architektura

**Version:** 0.1.0  
**Date:** 2025-11-09  
**Status:** ⚠️ Research Prototype (NOT Production-Ready)

> **DISCLAIMER:** This is a research implementation for NLnet grant application.  
> NOT audited, NOT optimized, NOT ready for production deployment.

---

## 📖 Language / Język

This document is bilingual (English / Polski).

---

## 🎯 System Overview / Przegląd Systemu

TRUE TRUST is a post-quantum blockchain with five core layers:

TRUE TRUST to post-kwantowy blockchain z pięcioma głównymi warstwami:

```
┌────────────────────────────────────────────────────────────┐
│                     APPLICATION LAYER                       │
│                    WARSTWA APLIKACJI                        │
│  Wallet CLI • Block Explorer • DApps • Smart Contracts     │
└────────────────────────┬───────────────────────────────────┘
                         │
┌────────────────────────┴───────────────────────────────────┐
│                     CONSENSUS LAYER                         │
│                    WARSTWA KONSENSUSU                       │
│  PoT (2/3 trust + 1/3 stake) • RandomX PoW • RTT • RANDAO │
└────────────────────────┬───────────────────────────────────┘
                         │
┌────────────────────────┴───────────────────────────────────┐
│                  CRYPTOGRAPHY LAYER                         │
│                WARSTWA KRYPTOGRAFICZNA                      │
│  Falcon512 • Kyber768 • STARK (Goldilocks*) • SHA3        │
└────────────────────────┬───────────────────────────────────┘
                         │
┌────────────────────────┴───────────────────────────────────┐
│                    PRIVACY LAYER                            │
│                  WARSTWA PRYWATNOŚCI                        │
│  Encrypted TX • STARK Range Proofs • Stealth Addresses    │
└────────────────────────┬───────────────────────────────────┘
                         │
┌────────────────────────┴───────────────────────────────────┐
│                    NETWORK LAYER                            │
│                   WARSTWA SIECIOWA                          │
│  PQ-Secure P2P • XChaCha20-Poly1305 • Replay Protection   │
└────────────────────────────────────────────────────────────┘
```

---

## 1️⃣ Consensus Layer / Warstwa Konsensusu

### 1.1 Proof-of-Trust (PoT)

**Core Concept / Główna Koncepcja:**

```rust
Weight = (2/3) × Trust + (1/3) × Stake

where:
  Trust = RTT_Algorithm(participation, quality, vouching, uptime)
  Stake = Time_Locked_UTXO_Balance
```

**Components / Komponenty:**

```
┌─────────────────────────────────────────────────────────┐
│ PoT Consensus Engine                                    │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  ┌──────────────┐   ┌──────────────┐   ┌────────────┐ │
│  │ Trust State  │   │ Stake State  │   │ RANDAO     │ │
│  │ (RTT Tree)   │   │ (UTXO-based) │   │ Beacon     │ │
│  └──────┬───────┘   └──────┬───────┘   └─────┬──────┘ │
│         │                  │                  │        │
│         └──────────────────┼──────────────────┘        │
│                            ▼                           │
│                 ┌─────────────────────┐                │
│                 │ Weight Calculation  │                │
│                 │   (Q32.32 Fixed)    │                │
│                 └──────────┬──────────┘                │
│                            ▼                           │
│                 ┌─────────────────────┐                │
│                 │ Leader Selection    │                │
│                 │ (Deterministic)     │                │
│                 └──────────┬──────────┘                │
│                            ▼                           │
│                 ┌─────────────────────┐                │
│                 │ RandomX PoW Check   │                │
│                 └─────────────────────┘                │
└─────────────────────────────────────────────────────────┘
```

### 1.2 Recursive Trust Tree (RTT)

**Algorithm / Algorytm:**

```rust
pub struct TrustState {
    map: HashMap<NodeId, TrustEntry>,
}

pub struct TrustEntry {
    trust_q: Q,                    // Q32.32 fixed-point trust score
    last_update: u64,              // Slot of last update
    ewma_history: EwmaHistory,     // Exponential weighted moving average
    vouched_by: Vec<(NodeId, Q)>,  // Trust vouching from peers
}

// Calculate trust using RTT algorithm
pub fn calculate_trust(
    participation: f64,   // Block production rate
    quality: f64,         // Quality metrics (fees, uptime, etc.)
    vouching: f64,        // Trust vouched by other validators
    decay: f64,           // Time-based decay for inactivity
) -> Q {
    // S-curve: S(x) = 3x² - 2x³
    let participation_trust = s_curve(participation);
    let quality_trust = s_curve(quality);
    let vouching_trust = capped_vouching(vouching); // Max 20%
    
    let raw_trust = 
        0.4 * participation_trust +
        0.4 * quality_trust +
        0.2 * vouching_trust;
    
    let decayed_trust = raw_trust * decay;
    
    q_from_f64(decayed_trust)
}
```

**Properties / Właściwości:**

- ✅ **Deterministic** - Q32.32 fixed-point arithmetic (no floating point)
- ✅ **Byzantine-Resistant** - Capped vouching (max 20%)
- ✅ **Time-Decay** - Inactive validators lose trust
- ✅ **EWMA History** - Smooth trust changes over time

### 1.3 RandomX Proof-of-Work

**Integration / Integracja:**

```rust
// RandomX used for CPU-fair mining
pub fn mine_block_pow(
    block_header: &BlockHeader,
    validator_weight: Q,
    beacon: &RandaoBeacon,
    difficulty: u64,
) -> Option<RandomXPoW> {
    let mut vm = randomx::Vm::new(...);
    let threshold = calculate_threshold(validator_weight, difficulty);
    
    for nonce in 0..u64::MAX {
        block_header.nonce = nonce;
        let hash = vm.calculate_hash(&block_header.serialize());
        
        if hash_to_u256(hash) < threshold {
            return Some(RandomXPoW { nonce, hash });
        }
    }
    
    None
}
```

**Properties / Właściwości:**

- ✅ **ASIC-Resistant** - Memory-hard algorithm
- ✅ **CPU-Fair** - Old CPUs have a chance
- ✅ **Monero-Compatible** - Uses tevador/RandomX library
- ✅ **Fast Verification** - ~5μs per hash

---

## 2️⃣ Cryptography Layer / Warstwa Kryptograficzna

### 2.1 Digital Signatures - Falcon512

**Algorithm / Algorytm:** NTRU lattice-based (NIST PQC Round 3)

```rust
pub struct FalconPublicKey([u8; 897]);
pub struct FalconSecretKey(Vec<u8>); // ~1281 bytes

// Sign block header
pub fn falcon_sign_block(
    sk: &FalconSecretKey,
    block_hash: &[u8; 32],
) -> BlockSignature {
    // Uses pqcrypto_falcon::falcon512
    let sig = falcon512::sign(block_hash, sk);
    BlockSignature(sig.as_bytes().to_vec())
}

// Verify signature
pub fn falcon_verify_block(
    pk: &FalconPublicKey,
    block_hash: &[u8; 32],
    sig: &BlockSignature,
) -> bool {
    let sig_obj = falcon512::DetachedSignature::from_bytes(&sig.0).ok()?;
    falcon512::verify(&sig_obj, block_hash, pk).is_ok()
}
```

**Performance / Wydajność:**

| Operation | Time | Size |
|-----------|------|------|
| Key Generation | ~5ms | PK: 897B, SK: 1281B |
| Sign | ~2ms | Signature: 690B (avg) |
| Verify | ~0.5ms | - |

**Security / Bezpieczeństwo:**

- Classical: 128-bit
- Quantum: 64-bit (NIST Level 1)

### 2.2 Key Exchange - Kyber768

**Algorithm / Algorytm:** Module-LWE lattice-based (NIST PQC Round 3 Winner)

```rust
pub struct KyberPublicKey(Vec<u8>);    // 1184 bytes
pub struct KyberSecretKey(Vec<u8>);    // 2400 bytes
pub struct KyberCiphertext(Vec<u8>);   // 1088 bytes
pub struct KyberSharedSecret([u8; 32]);

// Encapsulate (sender)
pub fn kyber_encapsulate(
    pk: &KyberPublicKey
) -> (KyberCiphertext, KyberSharedSecret) {
    let (ct, ss) = kyber768::encapsulate(pk);
    (KyberCiphertext(ct.as_bytes().to_vec()), 
     KyberSharedSecret(ss.as_bytes().try_into().unwrap()))
}

// Decapsulate (receiver)
pub fn kyber_decapsulate(
    ct: &KyberCiphertext,
    sk: &KyberSecretKey,
) -> Result<KyberSharedSecret> {
    let ct_obj = kyber768::Ciphertext::from_bytes(&ct.0)?;
    let ss = kyber768::decapsulate(&ct_obj, sk);
    Ok(KyberSharedSecret(ss.as_bytes().try_into()?))
}
```

**Performance / Wydajność:**

| Operation | Time | Size |
|-----------|------|------|
| Key Generation | ~2ms | PK: 1184B, SK: 2400B |
| Encapsulate | ~1ms | Ciphertext: 1088B |
| Decapsulate | ~1.5ms | Shared Secret: 32B |

**Security / Bezpieczeństwo:**

- Classical: 192-bit
- Quantum: 96-bit (NIST Level 3)

### 2.3 Zero-Knowledge Proofs - STARK

**Field / Pole:** Goldilocks Prime = `2^64 - 2^32 + 1`

```rust
pub const FIELD_MODULUS: u64 = 0xFFFFFFFF00000001;
pub const MAX_2_ADIC_ORDER: usize = 32;
pub const PRIMITIVE_ROOT: u64 = 7;

pub struct STARKProof {
    public_inputs: Vec<u64>,    // [value, commitment_0..3]
    trace_commitment: Vec<u8>,  // Merkle root (32B)
    constraint_commitment: Vec<u8>,
    fri_proof: FRIProof,
}

// Prove range with commitment binding
pub fn prove_range_with_commitment(
    value: u64,
    commitment: &Hash32,
) -> STARKProof {
    let public_inputs = encode_range_public_inputs(value, commitment);
    
    // 1. Encode execution trace
    let trace = encode_range_trace(value);
    
    // 2. Commit to trace (Merkle tree)
    let trace_commitment = merkle_commit(&trace);
    
    // 3. FRI low-degree test
    let fri_proof = fri_prove(&trace, &public_inputs);
    
    STARKProof {
        public_inputs,
        trace_commitment,
        constraint_commitment: vec![],
        fri_proof,
    }
}
```

**FRI Parameters / Parametry FRI:**

```rust
pub struct FRIConfig {
    blowup_factor: usize,   // 16 (Goldilocks)
    num_queries: usize,     // 80 (Goldilocks)
    fold_factor: usize,     // 4
}

// Security analysis
Soundness: ~160 bits (80 queries × 16 blowup)
Field collision: 64 bits (Goldilocks size)
Hash collision: 128 bits (SHA3-256)
Classical security: min(160, 64, 128) = 64 bits ✅
Quantum security: 64 / 2 = 32 bits ✅
```

**Performance / Wydajność (Unoptimized Research Code):**

| Field | Prove Time | Verify Time | Proof Size | Status |
|-------|------------|-------------|------------|--------|
| BabyBear (31-bit) | ~1-2s | ~200-500ms | ~100-400 KB | Testing only |
| Goldilocks (64-bit)* | ~2-4s | ~300-700ms | ~100-200 KB | **DEFAULT** |
| BN254 (256-bit) | Not implemented | - | - | Future |

*Current default. Production optimizations could achieve: 500ms-1s prove, 100-200ms verify, 50-100 KB proofs.

---

## 3️⃣ Privacy Layer / Warstwa Prywatności

### 3.1 Private Transactions

**Architecture / Architektura:**

```rust
pub struct TxOutputStark {
    // 1. Value commitment (SHA3)
    value_commitment: Hash32,  // SHA3(value || blinding || recipient)
    
    // 2. STARK range proof (0 ≤ value < 2^64)
    stark_proof: Vec<u8>,      // ~100-200 KB (unoptimized)
    
    // 3. Recipient (stealth address)
    recipient: Hash32,
    
    // 4. Encrypted value (Kyber + XChaCha20)
    encrypted_value: Vec<u8>,  // Kyber CT + XChaCha20-Poly1305
}
```

**Flow / Przepływ:**

```
Sender                           Recipient
  │                                 │
  │ 1. Get recipient Kyber PK       │
  ├────────────────────────────────>│
  │                                 │
  │ 2. Generate shared secret       │
  │    (Kyber KEM)                  │
  │                                 │
  │ 3. Encrypt (value, blinding)    │
  │    XChaCha20-Poly1305(value)    │
  │                                 │
  │ 4. Compute commitment           │
  │    c = SHA3(value||blind||to)   │
  │                                 │
  │ 5. Generate STARK proof         │
  │    prove_range_with_commitment  │
  │    (value, c) → proof (~50KB)   │
  │                                 │
  │ 6. Broadcast TX                 │
  ├────────────────────────────────>│
  │                                 │
  │ 7. Verify commitment binding    │
  │    (proof.commitment == TX.c)   │
  │                                 │
  │ 8. Verify STARK proof           │
  │    (100ms)                      │
  │                                 │
  │ 9. Decrypt value (Kyber SK)     │
  │<────────────────────────────────┤
  │                                 │
  │ 10. Verify commitment integrity │
  │     SHA3(decrypted) == c        │
  └─────────────────────────────────┘
```

### 3.2 Stealth Addresses

```rust
// Generate stealth address
pub fn generate_stealth_address(
    recipient_pk: &PublicKey,
    ephemeral_key: &[u8; 32],
    index: u64,
) -> Hash32 {
    let mut hasher = Sha3_256::new();
    hasher.update(recipient_pk.as_bytes());
    hasher.update(ephemeral_key);
    hasher.update(&index.to_le_bytes());
    hasher.finalize().into()
}

// Bloom filter for pre-filtering
pub fn check_stealth_address_bloom(
    address: &Hash32,
    bloom: &BloomFilter,
) -> bool {
    bloom.contains(address)
}
```

### 3.3 ZK Trust Proofs

```rust
// Privacy-preserving trust proof
// Prove: "I have trust_score ≥ threshold" without revealing exact score
pub fn prove_trust_threshold(
    trust_score: Q,
    threshold: Q,
    witness: TrustWitness,
) -> ZkTrustProof {
    // Lightweight ZK proof (not STARK - too heavy for this)
    // Uses commitment + challenge-response protocol
    hash_commitment_trust_proof(trust_score, witness)
}
```

---

## 4️⃣ Network Layer / Warstwa Sieciowa

### 4.1 PQ-Secure P2P Handshake

**3-Way Handshake / Potrójne Potwierdzenie:**

```
┌────────┐                                      ┌────────┐
│ Client │                                      │ Server │
└───┬────┘                                      └───┬────┘
    │                                               │
    │ ────── 1. ClientHello ──────────────────────> │
    │   {                                           │
    │     client_kyber_pk: [u8; 1184],             │
    │     client_falcon_pk: [u8; 897],             │
    │     timestamp: u64,                           │
    │     signature: Falcon(client_sk, msg)         │
    │   }                                           │
    │                                               │
    │ <────── 2. ServerHello ────────────────────── │
    │   {                                           │
    │     kyber_ciphertext: [u8; 1088],            │
    │     server_falcon_pk: [u8; 897],             │
    │     timestamp: u64,                           │
    │     signature: Falcon(server_sk, msg)         │
    │   }                                           │
    │                                               │
    │ ────── 3. ClientFinished ─────────────────> │
    │   {                                           │
    │     transcript_mac: KMAC256(session_key, T)   │
    │   }                                           │
    │                                               │
    │ ═══════ Encrypted Channel ══════════════════ │
    │   XChaCha20-Poly1305 AEAD                    │
    │   (24-byte nonce, 16-byte tag)               │
    └───────────────────────────────────────────────┘
```

**Security Properties / Właściwości Bezpieczeństwa:**

```
┌────────────────────────────────────────────────┐
│ Security Properties                            │
├────────────────────────────────────────────────┤
│ ✅ Mutual Authentication (Falcon512)          │
│ ✅ Forward Secrecy (Ephemeral Kyber keys)     │
│ ✅ Replay Protection (Transcript hashing)     │
│ ✅ Quantum-Resistant (No ECDH/RSA)            │
│ ✅ AEAD Encryption (XChaCha20-Poly1305)       │
│ ✅ Key Derivation (KMAC256)                   │
└────────────────────────────────────────────────┘
```

### 4.2 Message Protocol

```rust
pub enum P2pMessage {
    // Network management
    Ping,
    Pong,
    Status {
        height: u64,
        best_hash: Hash32,
        peer_count: usize,
    },
    
    // Block propagation
    NewBlock {
        block: Block,
        witness: LeaderWitness,
    },
    GetBlocks {
        start_height: u64,
        max_blocks: usize,
    },
    Blocks(Vec<Block>),
    
    // Transaction propagation
    NewTransaction(TransactionStark),
    GetMempool,
    Mempool(Vec<TransactionStark>),
}
```

---

## 5️⃣ Storage Layer / Warstwa Pamięci

### 5.1 Blockchain State

```rust
pub struct State {
    pub utxo_set: HashMap<Hash32, TxOutputStark>,
    pub spent_set: HashSet<Hash32>,
    pub address_index: HashMap<Hash32, Vec<Hash32>>, // address → UTXOs
}

pub struct StatePriv {
    pub my_outputs: Vec<(Hash32, u64, Vec<u8>)>, // (utxo_id, value, blinding)
    pub my_keys: HashMap<Hash32, KyberSecretKey>,
}
```

### 5.2 Block Structure

```rust
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<TransactionStark>,
}

pub struct BlockHeader {
    pub version: u32,
    pub height: u64,
    pub prev_hash: Hash32,
    pub merkle_root: Hash32,
    pub timestamp: u64,
    pub difficulty: u64,
    pub nonce: u64,
    pub miner: Hash32, // Falcon PK hash
}
```

---

## 📊 Performance Analysis / Analiza Wydajności

### Transaction Throughput / Przepustowość Transakcji

```
┌──────────────────────────────────────────────────────┐
│ Field        │ Block Time │ TX/Block │ TPS           │
├──────────────┼────────────┼──────────┼───────────────┤
│ BabyBear     │ 2.5s       │ 100      │ ~40 TPS       │
│ Goldilocks   │ 5.0s       │ 100      │ ~20 TPS       │
│ BN254        │ 50s        │ 100      │ ~2 TPS        │
└──────────────────────────────────────────────────────┘
```

### Memory Footprint / Zużycie Pamięci

```
Validator Node:
├─ Base memory: ~100 MB
├─ RandomX cache: ~2 MB
├─ RandomX dataset: ~2 GB
├─ Blockchain state: ~1-10 GB (depends on history)
└─ Total: ~4-13 GB

Wallet CLI:
├─ Base memory: ~50 MB
├─ Keys storage: ~5 KB
└─ Total: ~50 MB
```

---

## 🔐 Security Model / Model Bezpieczeństwa

### Threat Model / Model Zagrożeń

```
┌──────────────────────────────────────────────────────────┐
│ Attack Vector              │ Mitigation                  │
├────────────────────────────┼─────────────────────────────┤
│ Quantum Computer (Shor)    │ Falcon/Kyber (NIST PQC)    │
│ Quantum Computer (Grover)  │ 32-bit security (safe 2040)│
│ ASIC Mining                │ RandomX (memory-hard)       │
│ Sybil Attack               │ PoT trust + stake required  │
│ Double Spend               │ UTXO model + finality       │
│ Privacy Leak               │ STARK + encryption + stealth│
│ Network MITM               │ PQ-secure P2P handshake     │
│ Proof Reuse                │ Commitment binding          │
└──────────────────────────────────────────────────────────┘
```

### Byzantine Tolerance / Tolerancja Bizantyjska

```
PoT consensus can tolerate:
- Up to 1/3 Byzantine validators (trust-weighted)
- Up to 33% of total stake controlled by adversary
- Requires 2/3 trust-weighted quorum for finality
```

---

## 📚 References / Referencje

### Standards / Standardy

- **[NIST PQC](https://csrc.nist.gov/projects/post-quantum-cryptography)** - Falcon, Kyber
- **[SHA-3](https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.202.pdf)** - Keccak-256
- **[RandomX](https://github.com/tevador/RandomX)** - Proof-of-Work

### Research Papers / Prace Naukowe

- **[STARK](https://eprint.iacr.org/2018/046)** - Scalable Transparent Arguments of Knowledge
- **[FRI](https://drops.dagstuhl.de/opus/volltexte/2018/9018/)** - Fast Reed-Solomon IOP
- **[Falcon](https://falcon-sign.info/)** - Fast Fourier Lattice-based Compact Signatures
- **[Kyber](https://pq-crystals.org/kyber/)** - Module-LWE Key Encapsulation

---

**See also / Zobacz też:**
- [README.md](README.md) - Project overview
- [SECURITY.md](SECURITY.md) - Security policy
- [docs/consensus/](docs/consensus/) - Detailed consensus docs
- [docs/crypto/](docs/crypto/) - Cryptography documentation

---

<p align="center">
  <strong>Built with ❤️ for a quantum-safe future</strong><br>
  <strong>Zbudowane z ❤️ dla kwantowo-bezpiecznej przyszłości</strong>
</p>
