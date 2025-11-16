# 🖥️ CPU-Only Consensus Model

## Przegląd

TRUE TRUST używa **CPU-only** konsensusu łączącego:
1. **Proof-of-Trust (PoT)** - 2/3 wagi
2. **Proof-of-Stake (PoS)** - 1/3 wagi
3. **Micro PoW** - CPU-friendly difficulty
4. **Proof Generation** - Trust przez pracę (BP/ZK)

---

## 🎯 Model Wagi: 2/3 Trust + 1/3 Stake

```rust
weight = (2/3) × trust + (1/3) × stake
```

### Trust (2/3 wagi)
- **Zdobywany przez:** Generowanie dowodów (BP, ZK, PoW)
- **NIE przez:** Samo posiadanie bloków
- **Model:** Trust = f(proof_work)

### Stake (1/3 wagi)
- **Min requirement:** `min_stake_pos` (default: 1M coins)
- **Purpose:** Skin in the game, spam protection
- **Not dominant:** Only 33% of weight

---

## 🔧 Trust Building Model

### 1. **Bulletproofs (BP)**
```rust
trust_delta_bp = trust_per_bp × bp_count
default: 0.001 per BP
```
**Praca:**
- CPU-only: Ristretto scalar ops
- ~50-200ms per proof (64-bit range)
- Verification: ~5-10ms

### 2. **ZK Proofs**
```rust
trust_delta_zk = trust_per_zk × zk_count
default: 0.002 per ZK (2x BP)
```
**Praca:**
- RISC0 zkVM (CPU-only)
- ~1-5s per proof
- Higher reward due to complexity

### 3. **Micro PoW**
```rust
trust_delta_pow = trust_per_pow × sqrt(iterations) / 1000
default: 0.0001 base
```
**Praca:**
- SHAKE256 hash (CPU-friendly)
- Difficulty: 16-bit target (~65k hashes avg)
- NO GPU advantage (memory-hard possible)

---

## 📊 Trust Update Formula

```rust
fn apply_proof_trust_reward(
    trust_state: &mut TrustState,
    who: &NodeId,
    pot_params: &PotParams,
    bp_count: u32,
    zk_count: u32,
    pow_iterations: u64,
) {
    let current_trust = trust_state.get(who, init_q);
    
    // Calculate deltas
    let Δbp = trust_per_bp × bp_count
    let Δzk = trust_per_zk × zk_count
    let Δpow = trust_per_pow × (sqrt(pow_iterations) / 1000)
    
    // Apply
    new_trust = clamp(current_trust + Δbp + Δzk + Δpow, 0, 1)
    
    trust_state.set(who, new_trust);
}
```

---

## ⚙️ Micro PoW Details

### Algorithm
```rust
fn mine_micro_pow(data: &[u8], difficulty: u8) -> PowProof {
    for nonce in 0..max_iterations {
        hash = SHAKE256("MICRO_POW" || data || nonce)
        if leading_zeros(hash) >= difficulty {
            return PowProof { nonce, hash, iterations: nonce+1 }
        }
    }
    None
}
```

### Parameters
| Difficulty | Avg Iterations | Avg Time (CPU) |
|------------|----------------|----------------|
| 8-bit      | ~256           | <1ms           |
| 16-bit     | ~65,536        | ~10-50ms       |
| 20-bit     | ~1M            | ~100-500ms     |
| 24-bit     | ~16M           | ~2-10s         |

**Default:** 16-bit (good balance)

### CPU-Only Enforcement
- **SHAKE256:** No GPU advantage (not parallelizable like SHA256)
- **Memory access patterns:** Random (cache-unfriendly for GPUs)
- **Verification:** <1ms (very cheap)

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
    pub min_stake_pos: u64,  // Default: 1_000_000 (1M coins)
}
```

### Eligibility Check
```rust
fn check_pos_eligibility(validator_stake: u64, params: &PotParams) -> bool {
    validator_stake >= params.min_stake_pos
}
```

**Not eligible → can't mine, even with high trust!**

---

## 🔄 Mining Flow (CPU-Only)

```
┌─────────────────────────────────────────────────────────┐
│ 1. CHECK PoS ELIGIBILITY                                │
│    if stake < min_stake_pos → SKIP                      │
└─────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────┐
│ 2. CHECK PoT ELIGIBILITY                                │
│    weight = (2/3)×trust + (1/3)×stake                   │
│    if random() < weight × lambda → WIN                  │
└─────────────────────────────────────────────────────────┘
                           ↓ (if won)
┌─────────────────────────────────────────────────────────┐
│ 3. COLLECT TXs FROM MEMPOOL                             │
│    - Parse transactions                                 │
│    - Verify Bulletproofs (CPU: Ristretto)               │
│    - Count BP/ZK proofs                                 │
└─────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────┐
│ 4. MINE MICRO PoW (CPU: SHAKE256)                       │
│    data = parent_hash || height || txs_hash             │
│    proof = mine_micro_pow(data, difficulty)             │
│    → pow_iterations tracked                             │
└─────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────┐
│ 5. AGGREGATE ZK PROOFS (CPU: RISC0)                     │
│    - Drain child receipts                               │
│    - Aggregate (fanout: 1-64)                           │
│    → zk_count tracked                                   │
└─────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────┐
│ 6. CREATE BLOCK                                         │
│    - Assemble header + txs + zk_receipt + pow_proof    │
│    - Sign with Ed25519 (CPU: Dalek)                     │
└─────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────┐
│ 7. UPDATE TRUST (NEW MODEL!)                            │
│    apply_proof_trust_reward(                            │
│        trust_state,                                     │
│        node_id,                                         │
│        bp_count,    ← from TX verification              │
│        zk_count,    ← from aggregation                  │
│        pow_iterations ← from micro PoW                  │
│    )                                                    │
│    → trust increases based on WORK, not just block!    │
└─────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────┐
│ 8. BROADCAST BLOCK                                      │
└─────────────────────────────────────────────────────────┘
```

---

## 🎯 Example: Trust Building

### Scenario
Validator mines a block with:
- 50 TXs with Bulletproofs → `bp_count = 50`
- 1 ZK aggregation proof → `zk_count = 1`
- Micro PoW with 80,000 iterations → `pow_iterations = 80000`

### Calculation
```rust
// Current trust: 0.5
let current_trust = 0.5

// BP reward: 0.001 × 50 = 0.05
let Δbp = 0.001 × 50 = 0.05

// ZK reward: 0.002 × 1 = 0.002
let Δzk = 0.002 × 1 = 0.002

// PoW reward: 0.0001 × (sqrt(80000) / 1000) ≈ 0.0001 × 0.283 = 0.0000283
let Δpow = 0.0001 × (sqrt(80000) / 1000) ≈ 0.000028

// Total delta: 0.05 + 0.002 + 0.000028 ≈ 0.052
let total_delta = 0.052

// New trust: 0.5 + 0.052 = 0.552 (clamped to [0, 1])
let new_trust = clamp(0.5 + 0.052, 0, 1) = 0.552
```

**Trust increased by ~10% through proof work!**

---

## 🔐 Security Properties

### 1. **CPU-Only (No GPU)**
- **SHAKE256:** Serial algorithm, no parallelization benefit
- **Bulletproofs:** Scalar operations on Ristretto (CPU-friendly)
- **RISC0:** zkVM runs on CPU
- **Result:** Fair playing field for all nodes

### 2. **Trust Through Work**
- Can't fake proof generation
- Bulletproofs must verify correctly
- ZK proofs must be valid
- Micro PoW must meet difficulty target

### 3. **PoS Minimum Stake**
- Sybil attack expensive (need min_stake_pos per node)
- Spam protection (can't flood with 0-stake validators)
- Alignment (validators have financial stake)

### 4. **2/3 Trust Dominance**
- Trust (earned) > Stake (bought)
- Encourages long-term participation
- Rewards actual work

---

## 📈 Tuning Parameters

### Default Configuration
```rust
PotParams {
    // PoS
    min_stake_pos: 1_000_000,  // 1M coins minimum
    
    // Trust rewards
    trust_per_bp: 0.001,       // 0.1% per Bulletproof
    trust_per_zk: 0.002,       // 0.2% per ZK proof
    trust_per_pow: 0.0001,     // 0.01% base per PoW
    
    // Micro PoW
    pow_difficulty: 16,        // 16-bit (65k avg iterations)
    pow_max_iterations: 1_000_000,
}
```

### Adjusting Difficulty
- **Lower (8-12 bit):** Faster blocks, less CPU cost
- **Higher (20-24 bit):** Slower blocks, more decentralization
- **Sweet spot:** 16-bit (~50ms per block)

### Adjusting Trust Rewards
- **Higher rewards:** Faster trust building, more weight on work
- **Lower rewards:** Slower trust accumulation, more stable
- **Ratio BP:ZK:PoW = 10:20:1** (ZK hardest, gets most reward)

---

## 🚀 Why This Model?

### Traditional PoW (Bitcoin)
- ❌ GPU/ASIC dominated
- ❌ Massive energy waste
- ❌ Centralization (mining pools)

### Pure PoS
- ❌ Rich get richer
- ❌ Nothing at stake
- ❌ Low participation incentive

### TRUE TRUST Model
- ✅ **CPU-only:** Fair for all
- ✅ **Trust through work:** Earn influence
- ✅ **PoS minimum:** Skin in the game
- ✅ **Micro PoW:** Light spam protection
- ✅ **2/3 Trust:** Work > wealth

---

## 🔄 Migration Path

### Phase 1 (Current)
- PoT + PoS (2/3 + 1/3)
- Trust via block production

### Phase 2 (This PR)
- **Add:** Micro PoW (16-bit)
- **Add:** Trust via proof generation
- **Add:** Min stake requirement

### Phase 3 (Future)
- **Optimize:** Memory-hard PoW (further GPU resistance)
- **Add:** Proof aggregation incentives
- **Add:** Dynamic difficulty adjustment

---

## 📝 Summary

**TRUE TRUST Consensus:**
```
Eligibility = (stake >= min_stake_pos) AND (weight > random_threshold)
Weight = (2/3) × trust + (1/3) × stake
Trust = f(BP_work, ZK_work, PoW_work)
```

**Key Properties:**
1. 🖥️ CPU-only (no GPU advantage)
2. 💪 Trust earned through proof work
3. 💰 Min stake for participation
4. ⚖️ 2/3 trust, 1/3 stake (work > wealth)
5. 🔒 Micro PoW for spam protection

**Result:** Fair, decentralized, work-based consensus! 🎉
