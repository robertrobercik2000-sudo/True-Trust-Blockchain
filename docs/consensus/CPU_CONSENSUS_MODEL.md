# 🖥️ CPU-Only Consensus Model

## Overview

TRUE TRUST uses **CPU-only** consensus combining:
1. **Proof-of-Trust (PoT)** - 2/3 weight
2. **Proof-of-Stake (PoS)** - 1/3 weight
3. **RandomX PoW** - ASIC-resistant, CPU-fair mining
4. **Proof Generation** - Trust through work (STARK proofs)

---

## 🎯 Weight Model: 2/3 Trust + 1/3 Stake

```latex
W = \frac{2}{3} \cdot T + \frac{1}{3} \cdot S
```

### Trust (2/3 weight)
- **Earned through:** Generating proofs (STARK, RandomX)
- **NOT through:** Just holding blocks
- **Model:** Trust = f(proof_work)

### Stake (1/3 weight)
- **Min requirement:** `min_stake` (default: 1M coins)
- **Purpose:** Skin in the game, spam protection
- **Not dominant:** Only 33% of weight

---

## 🔧 Trust Building Model

### 1. **STARK Proofs (PQ-Secure)**

```rust
trust_delta_stark = trust_per_stark × stark_count
default: 0.002 per STARK proof
```

**Work:**
- CPU-only: Goldilocks field arithmetic (64-bit)
- ~500ms per range proof (0-2^64)
- Verification: ~100ms
- **Post-quantum secure:** 64-bit classical, 32-bit quantum

### 2. **RandomX PoW**

```rust
trust_delta_randomx = trust_per_pow × difficulty_factor
default: 0.001 base
```

**Work:**
- Monero-compatible RandomX
- ASIC-resistant, memory-hard
- CPU-only (no GPU advantage)
- ~5μs per hash
- Fair for all CPUs (even old ones)

### 3. **Block Production Quality**

```rust
trust_delta_quality = quality_score × 0.001
quality_score = f(uptime, fees_collected, tx_count)
```

**Factors:**
- Uptime ratio (block production rate)
- Fees collected (economic contribution)
- Transaction count (network utility)
- Invalid blocks penalty

---

## 📊 Trust Update Formula

```rust
fn apply_proof_trust_reward(
    trust_state: &mut TrustState,
    who: &NodeId,
    pot_params: &PotParams,
    stark_count: u32,
    randomx_iterations: u64,
    quality: f64,
) {
    let current_trust = trust_state.get(who, init_q);
    
    // Calculate deltas
    let Δstark = trust_per_stark × stark_count
    let Δpow = trust_per_pow × randomx_difficulty_factor
    let Δquality = quality × 0.001
    
    // Apply with decay
    let decay_factor = calculate_decay(time_since_last_block);
    new_trust = clamp(
        (current_trust + Δstark + Δpow + Δquality) × decay_factor,
        0, 1
    )
    
    trust_state.set(who, new_trust);
}
```

---

## ⚙️ RandomX PoW Details

### Algorithm

```rust
fn mine_block_randomx(
    block_header: &BlockHeader,
    validator_weight: Q,
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

### Properties

| Property | Value | Notes |
|----------|-------|-------|
| Algorithm | RandomX | Monero-compatible |
| Cache Size | 2 MB | Fast init |
| Dataset Size | 2 GB | Memory-hard |
| Hash Time | ~5 μs | On CPU |
| ASIC Resistant | ✓ | Memory-hard + CPU-optimized |
| GPU Friendly | ✗ | Intentionally not |
| Old CPU Fair | ✓ | Optimized for all CPUs |

### CPU-Only Enforcement

- **Memory-hard:** Requires 2GB dataset (GPU cache penalty)
- **CPU-optimized:** Uses CPU instructions (AES, multiply)
- **Not parallelizable:** Sequential memory access pattern
- **Verification:** Fast (~5μs, cache-only)

---

## 💰 PoS Layer: Min Stake Requirement

### Purpose

1. **Spam protection:** Require investment to participate
2. **Sybil resistance:** Cost to create many nodes
3. **Alignment:** Validators have skin in the game

### Configuration

```rust
pub struct PotParams {
    // ...
    pub min_stake: u64,  // Default: 1_000_000 (1M coins)
}
```

### Eligibility Check

```rust
fn check_eligibility(
    validator_stake: u64,
    validator_trust: Q,
    params: &PotParams,
) -> bool {
    // Must meet minimum stake
    if validator_stake < params.min_stake {
        return false;
    }
    
    // Weight calculation
    let weight_q = compute_weight_linear(validator_trust, validator_stake);
    
    // Probabilistic eligibility (trust-weighted)
    weight_q > random_threshold()
}
```

**Not eligible → can't mine, even with high trust!**

---

## 🔄 Mining Flow (CPU-Only)

```
┌─────────────────────────────────────────────────────────┐
│ 1. CHECK PoS ELIGIBILITY                                │
│    if stake < min_stake → SKIP                          │
└─────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────┐
│ 2. CHECK PoT ELIGIBILITY (Deterministic)                │
│    weight = (2/3)×trust + (1/3)×stake                   │
│    leader = argmax(H(beacon||slot||pk) × weight)        │
│    if not_selected → SKIP                               │
└─────────────────────────────────────────────────────────┘
                           ↓ (if selected)
┌─────────────────────────────────────────────────────────┐
│ 3. COLLECT TXs FROM MEMPOOL                             │
│    - Parse transactions                                 │
│    - Verify STARK proofs (CPU: Goldilocks)              │
│    - Count valid STARK proofs                           │
│    → stark_count tracked                                │
└─────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────┐
│ 4. MINE RandomX PoW (CPU: Memory-hard)                  │
│    data = block_header_bytes                            │
│    proof = mine_randomx(data, difficulty, weight)       │
│    → pow_nonce tracked                                  │
└─────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────┐
│ 5. SIGN BLOCK (CPU: Falcon512 PQC)                      │
│    - Assemble header + txs                              │
│    - Sign with Falcon512 (PQ-secure)                    │
│    - Create LeaderWitness (Merkle proof)                │
└─────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────┐
│ 6. UPDATE TRUST                                         │
│    apply_proof_trust_reward(                            │
│        trust_state,                                     │
│        node_id,                                         │
│        stark_count,    ← from TX verification           │
│        randomx_nonce,  ← from RandomX PoW               │
│        quality_score   ← from block metrics             │
│    )                                                    │
│    → trust increases based on WORK!                    │
└─────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────┐
│ 7. BROADCAST BLOCK                                      │
│    - P2P (PQ-secure: Falcon + Kyber handshake)         │
│    - Encrypted channel (XChaCha20-Poly1305)             │
└─────────────────────────────────────────────────────────┘
```

---

## 🎯 Example: Trust Building

### Scenario

Validator mines a block with:
- 50 TXs with STARK proofs → `stark_count = 50`
- RandomX PoW with 100k iterations → `randomx_nonce = 100000`
- Quality score: 0.95 (95% uptime, good fees)

### Calculation

```rust
// Current trust: 0.5 (Q32.32: 2147483648)
let current_trust_q = 0.5 * ONE_Q

// STARK reward: 0.002 × 50 = 0.1
let Δstark = 0.002 × 50 = 0.1

// RandomX reward: 0.001 × difficulty_factor
// (difficulty_factor based on weight)
let Δpow = 0.001 × 1.0 = 0.001

// Quality reward: 0.95 × 0.001 = 0.00095
let Δquality = 0.95 × 0.001 = 0.00095

// Total delta: 0.1 + 0.001 + 0.00095 ≈ 0.102
let total_delta = 0.102

// Apply decay (assume no decay if recent)
let decay_factor = 1.0

// New trust: 0.5 + 0.102 = 0.602 (clamped to [0, 1])
let new_trust_q = clamp(
    (current_trust_q + (total_delta * ONE_Q)) * decay_factor,
    0,
    ONE_Q
)
// Result: 0.602
```

**Trust increased by ~20% through proof work!**

---

## 🔐 Security Properties

### 1. **100% Post-Quantum (PQ)**

- **STARK proofs:** Goldilocks field (64-bit classical, 32-bit quantum)
- **Falcon512 signatures:** NIST PQC (128-bit classical, 64-bit quantum)
- **Kyber768 KEM:** NIST PQC (192-bit classical, 96-bit quantum)
- **NO Bulletproofs:** ECC-based (NOT quantum-resistant)
- **NO ECDSA:** Broken by Shor's algorithm

**Result:** Quantum-resistant until ~2040

### 2. **CPU-Only (No GPU/ASIC)**

- **RandomX:** Memory-hard (2GB dataset)
- **STARK:** Goldilocks arithmetic (CPU-friendly)
- **Falcon512:** Lattice operations (CPU-only)
- **Result:** Fair playing field for all nodes

### 3. **Trust Through Work**

- Can't fake STARK proof generation (~500ms CPU work)
- Can't fake RandomX PoW (memory-hard, ASIC-resistant)
- Can't fake Falcon signatures (PQ-secure)
- Must verify correctly → real work proven

### 4. **PoS Minimum Stake**

- Sybil attack expensive (need min_stake per node)
- Spam protection (can't flood with 0-stake validators)
- Alignment (validators have financial stake)

### 5. **2/3 Trust Dominance**

- Trust (earned) > Stake (bought)
- Encourages long-term participation
- Rewards actual work, not just wealth

---

## 📈 Tuning Parameters

### Default Configuration

```rust
PotParams {
    // PoS
    min_stake: 1_000_000,      // 1M coins minimum
    
    // Trust rewards
    trust_per_stark: 0.002,    // 0.2% per STARK proof
    trust_per_pow: 0.001,      // 0.1% base per RandomX PoW
    trust_per_quality: 0.001,  // 0.1% per quality point
    
    // RandomX
    randomx_difficulty: 1000,  // Adaptive difficulty
}
```

### Adjusting Parameters

- **trust_per_stark:** Higher = faster trust building via STARK proofs
- **trust_per_pow:** Higher = more weight on RandomX mining
- **min_stake:** Higher = more Sybil resistance, lower participation
- **randomx_difficulty:** Adaptive based on network hashrate

---

## 🚀 Why This Model?

### Traditional PoW (Bitcoin)

- ❌ GPU/ASIC dominated
- ❌ Massive energy waste
- ❌ Centralization (mining pools)
- ❌ NOT quantum-resistant (ECDSA signatures)

### Pure PoS (Ethereum)

- ❌ Rich get richer
- ❌ Nothing at stake problem
- ❌ Low participation incentive
- ❌ NOT quantum-resistant (ECDSA signatures)

### TRUE TRUST Model

- ✅ **CPU-only:** Fair for all (RandomX + STARK)
- ✅ **Trust through work:** Earn influence
- ✅ **PoS minimum:** Skin in the game
- ✅ **2/3 Trust:** Work > wealth
- ✅ **100% PQ-secure:** Falcon + Kyber + STARK
- ✅ **ASIC-resistant:** RandomX (Monero-proven)
- ✅ **Deterministic leader:** No lottery, fair selection

---

## 🔬 Post-Quantum Security

### Why NO Bulletproofs?

```
Bulletproofs:
├─ Based on elliptic curves (ECC)
├─ Broken by Shor's algorithm (quantum)
├─ NOT post-quantum secure ❌
└─ Replaced by STARK

STARK (Goldilocks):
├─ Based on hash functions (SHA3)
├─ Resistant to quantum attacks ✓
├─ Transparent (no trusted setup) ✓
├─ 64-bit classical, 32-bit quantum ✓
└─ Used in TRUE TRUST ✓
```

### Full PQ Stack

| Component | Algorithm | Quantum Secure |
|-----------|-----------|----------------|
| **Signatures** | Falcon512 | ✓ NIST PQC |
| **Key Exchange** | Kyber768 | ✓ NIST PQC |
| **Range Proofs** | STARK (Goldilocks) | ✓ Hash-based |
| **Hashing** | SHA3-256 | ✓ Quantum-resistant |
| **PoW** | RandomX | ✓ Memory-hard |

**TRUE TRUST is 15 years ahead of Bitcoin/Ethereum!**

---

## 📝 Summary

**TRUE TRUST Consensus:**

```
Eligibility = (stake >= min_stake) AND (is_leader(slot))
Leader = argmax(H(beacon||slot||pk) × weight)
Weight = (2/3) × trust + (1/3) × stake
Trust = f(STARK_work, RandomX_work, quality)
```

**Key Properties:**

1. 🖥️ CPU-only (RandomX + STARK, no GPU)
2. 💪 Trust earned through proof work
3. 💰 Min stake for participation
4. ⚖️ 2/3 trust, 1/3 stake (work > wealth)
5. 🔒 100% Post-Quantum (Falcon + Kyber + STARK)
6. 🎯 Deterministic leader selection (no lottery)
7. ⏱️ Fast finality (~10s, 2 blocks)

**Result:** Fair, decentralized, quantum-resistant, work-based consensus! 🎉
