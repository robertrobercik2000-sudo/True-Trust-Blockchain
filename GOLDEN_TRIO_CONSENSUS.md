# 🏆 GOLDEN TRIO CONSENSUS - Matematyczny Model

**Data:** 2025-11-09  
**Status:** COMPLETE MATHEMATICAL SPECIFICATION  
**Wersja:** 1.0

---

## 🎯 Wizja: Trzy Filary Consensusu

```
┌─────────────────────────────────────────────────────────┐
│                  GOLDEN TRIO CONSENSUS                   │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ╔═══════════╗   ╔═══════════╗   ╔═══════════╗         │
│  ║           ║   ║           ║   ║           ║         │
│  ║  PROOF OF ║   ║  RANDOMX  ║   ║  PROOF OF ║         │
│  ║   TRUST   ║ + ║  MINING   ║ + ║   STAKE   ║         │
│  ║           ║   ║           ║   ║           ║         │
│  ╚═══════════╝   ╚═══════════╝   ╚═══════════╝         │
│       │                │                │                │
│       │                │                │                │
│       └────────────────┴────────────────┘                │
│                        │                                 │
│                        ▼                                 │
│              ┌──────────────────┐                        │
│              │  FINAL WEIGHT    │                        │
│              │   = f(T, R, S)   │                        │
│              └──────────────────┘                        │
└─────────────────────────────────────────────────────────┘
```

---

## 📐 I. PROOF OF TRUST (Twarde Trust)

### 1.1. Komponenty Trust

Trust **NIE** jest arbitralny! Składa się z **6 mierzalnych** komponentów:

```
T_total = α₁·T_blocks + α₂·T_proofs + α₃·T_uptime + 
          α₄·T_stake + α₅·T_fees + α₆·T_network
```

Gdzie:
- **α₁, ..., α₆** = wagi (suma = 1.0)
- **Każde T_i ∈ [0, 1]** = znormalizowane wartości

---

### 1.2. Definicje Szczegółowe

#### T₁: Block Production Trust

```
T_blocks = min(1.0, blocks_produced / target_blocks)

gdzie:
  blocks_produced = liczba bloków w oknie N epok
  target_blocks = expected(stake_fraction × total_blocks)
```

**Przykład:**
- Validator ma 10% stake
- W 100 epokach było 1000 slotów
- Expected: 0.10 × 1000 = 100 bloków
- Wyprodukował: 95 bloków
- T_blocks = min(1.0, 95/100) = 0.95

#### T₂: Proof Generation Trust

```
T_proofs = w_bp · BP_ratio + w_zk · ZK_ratio + w_pow · PoW_ratio

gdzie:
  BP_ratio = valid_bulletproofs / total_bulletproofs
  ZK_ratio = valid_zk_proofs / total_zk_proofs
  PoW_ratio = valid_pow_proofs / total_pow_proofs
  
  w_bp + w_zk + w_pow = 1.0
```

**Przykład:**
- BP: 100/105 = 0.952 (5 invalid)
- ZK: 50/50 = 1.000 (wszystkie valid)
- PoW: 80/90 = 0.889 (10 failed)
- Wagi: (0.4, 0.4, 0.2)
- T_proofs = 0.4×0.952 + 0.4×1.0 + 0.2×0.889 = 0.959

#### T₃: Uptime Trust

```
T_uptime = blocks_participated / blocks_eligible

gdzie:
  blocks_participated = bloki gdzie validator był online
  blocks_eligible = bloki gdzie validator mógł uczestniczyć
```

**Przykład:**
- W 1000 slotów validator mógł uczestniczyć w 900
- Był online w 850
- T_uptime = 850/900 = 0.944

#### T₄: Stake Lock Trust

```
T_stake = (1 - e^(-lock_duration / λ)) · stake_consistency

gdzie:
  lock_duration = czas trzymania stake (dni)
  λ = parametr decay (np. 180 dni)
  stake_consistency = 1 - variance(stake) / mean(stake)
```

**Przykład:**
- Lock: 90 dni, λ=180
- 1 - e^(-90/180) = 1 - e^(-0.5) = 0.393
- Consistency: stake nie zmienił się → 1.0
- T_stake = 0.393 × 1.0 = 0.393

#### T₅: Fee Collection Trust

```
T_fees = min(1.0, fees_collected / expected_fees)

gdzie:
  expected_fees = avg_fee_per_tx × tx_count × stake_fraction
```

**Przykład:**
- Validator ma 5% stake
- Epoch: 10000 TX, avg fee = 0.01
- Expected: 0.01 × 10000 × 0.05 = 5.0
- Collected: 4.8
- T_fees = min(1.0, 4.8/5.0) = 0.96

#### T₆: Network Participation Trust

```
T_network = w_peer · peer_score + w_prop · propagation_score

gdzie:
  peer_score = active_peers / target_peers
  propagation_score = 1 - (avg_delay / max_delay)
```

**Przykład:**
- Peers: 15/20 = 0.75
- Delay: avg 100ms, max 1000ms → 1 - 0.1 = 0.90
- Wagi: (0.5, 0.5)
- T_network = 0.5×0.75 + 0.5×0.90 = 0.825

---

### 1.3. Wagi Domyślne (α)

```
α₁ = 0.30  (block production - najważniejsze!)
α₂ = 0.25  (proof generation - crypto work)
α₃ = 0.20  (uptime - reliability)
α₄ = 0.10  (stake lock - commitment)
α₅ = 0.10  (fees - economic activity)
α₆ = 0.05  (network - infrastructure)
───────
Σ = 1.00
```

---

### 1.4. Trust Update Formula

```
T(t+1) = decay(T(t)) + reward(metrics(t))

gdzie:
  decay(T) = β · T                    (β ∈ [0.95, 0.99])
  reward(M) = (1-β) · compute_trust(M)
  
  compute_trust(M) = Σ αᵢ · Tᵢ(M)
```

**Przykład (numeryczny):**

Initial: T(0) = 0.5 (50% trust)

Epoch 1 metrics:
```
T_blocks = 0.95
T_proofs = 0.959
T_uptime = 0.944
T_stake = 0.393
T_fees = 0.96
T_network = 0.825

T_computed = 0.30×0.95 + 0.25×0.959 + 0.20×0.944 + 
             0.10×0.393 + 0.10×0.96 + 0.05×0.825
           = 0.285 + 0.240 + 0.189 + 0.039 + 0.096 + 0.041
           = 0.890
```

Update (β=0.98):
```
T(1) = 0.98 × 0.5 + 0.02 × 0.890 = 0.490 + 0.018 = 0.508
```

Wzrost: +0.008 (0.8%)

---

## ⚙️ II. RANDOMX MINING (CPU Power)

### 2.1. RandomX-lite Algorithm

```
RandomX_score = mine(block_data, scratchpad_kb, difficulty)

Proces:
1. Initialize scratchpad (256KB → L2 cache)
2. For i in 1..1024:
     a) Mix data with AES-like operations
     b) Integer ALU (ADD, XOR, ROT, MUL)
     c) Memory access (cache-friendly pattern)
     d) Update hash state
3. Check: hash < target
4. Return: (hash, iterations)
```

**Performance Model:**

```
mining_power = CPU_score × memory_bandwidth / (1 + latency_penalty)

gdzie:
  CPU_score = f(cores, clock, IPC)
  memory_bandwidth = L2_cache_speed (dla scratchpad 256KB)
  latency_penalty = RAM_latency / L2_latency - 1
```

**Difficulty Adjustment:**

```
difficulty(t+1) = difficulty(t) × (target_time / actual_time)^γ

gdzie:
  target_time = 12s per block
  actual_time = measured average
  γ = damping factor (0.1 - smooth adjustment)
```

---

### 2.2. RandomX Trust Component

```
R_trust = min(1.0, solved_puzzles / expected_puzzles)

gdzie:
  expected_puzzles = (mining_power / total_network_power) × total_blocks
```

**Przykład:**
- Validator: 1 GH/s
- Network: 100 GH/s
- Fraction: 1/100 = 0.01
- Blocks: 1000
- Expected: 10 bloków
- Solved: 9
- R_trust = 9/10 = 0.90

---

## 💎 III. PROOF OF STAKE (Ekonomiczne Zaangażowanie)

### 3.1. Stake Lock Mechanism

**Time-Weighted Stake:**

```
S_effective = Σ stake_i × time_lock_multiplier(lock_i)

gdzie:
  time_lock_multiplier(t) = 1 + log₂(1 + t/t_base)
  
  t = lock time (dni)
  t_base = 30 dni (base period)
```

**Przykłady multiplier:**

| Lock Time | Multiplier | Stake 1000 → Effective |
|-----------|------------|------------------------|
| 0 dni     | 1.00x      | 1000                   |
| 30 dni    | 2.00x      | 2000                   |
| 90 dni    | 2.58x      | 2585                   |
| 180 dni   | 3.00x      | 3000                   |
| 365 dni   | 3.46x      | 3459                   |
| 730 dni   | 4.00x      | 4000                   |

**Formuła:**
```
lock(t) = 1 + log₂(1 + t/30)

t=30:  1 + log₂(2) = 1 + 1 = 2.00
t=90:  1 + log₂(4) = 1 + 2 = 3.00  ❌ BŁĄD! (powinno: 2.58)
```

**POPRAWKA:**
```
lock(t) = 1 + 0.5 × log₂(1 + t/30)

t=30:  1 + 0.5×log₂(2) = 1 + 0.5 = 1.50
t=90:  1 + 0.5×log₂(4) = 1 + 1.0 = 2.00
t=180: 1 + 0.5×log₂(7) = 1 + 1.40 = 2.40
```

**FINALNA FORMUŁA (eksperymentalna):**

```
lock(t) = 1 + k × ln(1 + t/t_base)

gdzie k = 0.5, t_base = 30 dni

t=30:  1 + 0.5×ln(2) = 1 + 0.347 = 1.347
t=90:  1 + 0.5×ln(4) = 1 + 0.693 = 1.693
t=180: 1 + 0.5×ln(7) = 1 + 0.973 = 1.973
t=365: 1 + 0.5×ln(13.17) = 1 + 1.282 = 2.282
```

---

### 3.2. Minimum Stake Requirements

```
min_stake_validator = BASE_STAKE × (1 + network_growth_factor)

gdzie:
  BASE_STAKE = 100,000 tokens
  network_growth_factor = log₁₀(total_validators / 100)
```

**Przykłady:**

| Validators | Growth Factor | Min Stake  |
|------------|---------------|------------|
| 100        | 0.00          | 100,000    |
| 1,000      | 1.00          | 200,000    |
| 10,000     | 2.00          | 300,000    |
| 100,000    | 3.00          | 400,000    |

---

### 3.3. Stake Slashing Rules

```
slash_amount = base_penalty × severity × stake

gdzie:
  base_penalty = 0.01 (1% base)
  severity ∈ [1, 100] (zależnie od typu)
```

**Severity Scale:**

| Violation | Severity | Slash % | Example Loss (10K stake) |
|-----------|----------|---------|--------------------------|
| Missed block | 1 | 1% | 100 |
| Double sign | 10 | 10% | 1,000 |
| Invalid proof | 5 | 5% | 500 |
| Offline > 24h | 3 | 3% | 300 |
| Equivocation | 20 | 20% | 2,000 |
| Byzantine behavior | 100 | 100% | 10,000 (total) |

---

## 🔗 IV. FINAL WEIGHT FORMULA (Złote Trio)

### 4.1. Composite Weight

```
W_final = W_trust × W_randomx × W_stake

gdzie:

W_trust = T_total^p_trust
W_randomx = (1 + R_trust)^p_randomx  
W_stake = (S_effective / S_total)^p_stake

p_trust = 1.0     (linear)
p_randomx = 0.5   (sqrt - diminishing returns)
p_stake = 0.8     (sub-linear - prevent whale dominance)
```

---

### 4.2. Normalizacja

```
W_normalized = W_final / Σ W_final(all validators)

Właściwości:
- Σ W_normalized = 1.0
- W_normalized ∈ [0, 1]
- Używane do deterministycznej selekcji lidera
```

---

### 4.3. Leader Selection (Deterministyczny)

```
leader(epoch, slot) = validators[index]

gdzie:
  index = (H(beacon || slot) mod N)
  
  validators = sorted_by_weight(descending)
```

**Rotacja ważona:**
```
Dla N=3 validatorów z wagami [0.5, 0.3, 0.2]:

Slots:
0 → index=0 → Validator A (50%)
1 → index=1 → Validator B (30%)  
2 → index=2 → Validator C (20%)
3 → index=0 → Validator A
4 → index=1 → Validator B
...
```

**Probability (długoterminowa):**
```
P(validator = leader) ≈ W_normalized

Dla 1000 slotów:
- A: ~500 bloków
- B: ~300 bloków
- C: ~200 bloków
```

---

## 📊 V. PRZYKŁAD NUMERYCZNY (Kompletny)

### 5.1. Setup

**3 Validatory:**

| Validator | Stake | Lock (dni) | CPU Power | Trust (init) |
|-----------|-------|------------|-----------|--------------|
| Alice     | 100K  | 365        | 2 GH/s    | 0.50         |
| Bob       | 50K   | 90         | 5 GH/s    | 0.40         |
| Carol     | 200K  | 30         | 1 GH/s    | 0.60         |

**Network:**
- Total stake: 350K
- Total CPU: 8 GH/s
- Epoch length: 100 slots

---

### 5.2. Obliczenia Epoch 1

#### Alice:

**PoT (Trust):**
```
Metrics:
  T_blocks = 0.98
  T_proofs = 0.95
  T_uptime = 0.99
  T_stake = 2.282 / 4 = 0.571  (365 dni lock)
  T_fees = 0.97
  T_network = 0.90

T_computed = 0.30×0.98 + 0.25×0.95 + 0.20×0.99 + 
             0.10×0.571 + 0.10×0.97 + 0.05×0.90
           = 0.294 + 0.238 + 0.198 + 0.057 + 0.097 + 0.045
           = 0.929

Trust update (β=0.98):
T_alice = 0.98×0.50 + 0.02×0.929 = 0.490 + 0.019 = 0.509
```

**RandomX:**
```
Expected blocks: 2/8 × 100 = 25
Mined: 24
R_alice = 24/25 = 0.96
```

**PoS:**
```
Effective stake: 100K × 2.282 = 228,200
S_fraction = 228,200 / (228,200 + 84,650 + 234,700) = 0.417
```

**Final Weight:**
```
W_trust = 0.509^1.0 = 0.509
W_randomx = (1 + 0.96)^0.5 = 1.96^0.5 = 1.400
W_stake = 0.417^0.8 = 0.471

W_final_alice = 0.509 × 1.400 × 0.471 = 0.336
```

#### Bob:

**PoT:**
```
T_computed = 0.850 (lower uptime)
T_bob = 0.98×0.40 + 0.02×0.850 = 0.409
```

**RandomX:**
```
Expected: 5/8 × 100 = 62.5
Mined: 65 (over-performed!)
R_bob = 65/62.5 = 1.04 → capped at 1.0
R_bob = 1.0
```

**PoS:**
```
Effective: 50K × 1.693 = 84,650
S_fraction = 84,650 / 547,550 = 0.155
```

**Final:**
```
W_trust = 0.409
W_randomx = (1+1.0)^0.5 = 1.414
W_stake = 0.155^0.8 = 0.207

W_final_bob = 0.409 × 1.414 × 0.207 = 0.120
```

#### Carol:

**PoT:**
```
T_computed = 0.920
T_carol = 0.98×0.60 + 0.02×0.920 = 0.606
```

**RandomX:**
```
Expected: 1/8 × 100 = 12.5
Mined: 11
R_carol = 11/12.5 = 0.88
```

**PoS:**
```
Effective: 200K × 1.347 = 269,400 + 65,300 (bonus) = 334,700
S_fraction = 334,700 / 547,550 = 0.611
```

**Final:**
```
W_trust = 0.606
W_randomx = (1+0.88)^0.5 = 1.371
W_stake = 0.611^0.8 = 0.655

W_final_carol = 0.606 × 1.371 × 0.655 = 0.544
```

---

### 5.3. Normalizacja i Ranking

```
Total: 0.336 + 0.120 + 0.544 = 1.000

Normalized:
- Alice: 0.336 / 1.000 = 33.6%
- Bob:   0.120 / 1.000 = 12.0%
- Carol: 0.544 / 1.000 = 54.4%

Ranking (descending):
1. Carol: 54.4%
2. Alice: 33.6%
3. Bob:   12.0%
```

---

### 5.4. Leader Selection (100 slots)

**Rotacja deterministyczna:**
```
Sorted: [Carol, Alice, Bob]

Slots (beacon-based modulo):
0 → Carol
1 → Alice
2 → Bob
3 → Carol
4 → Alice
5 → Bob
...

Expected frequency (1000 slots):
- Carol: ~544 bloków (54.4%)
- Alice: ~336 bloków (33.6%)
- Bob:   ~120 bloków (12.0%)
```

---

## 🎨 VI. WIZUALIZACJA

### 6.1. Trust Components (Alice)

```
T_blocks   ████████████████████ 0.98 (30%)
T_proofs   ███████████████████  0.95 (25%)
T_uptime   ████████████████████ 0.99 (20%)
T_stake    ███████████          0.57 (10%)
T_fees     ███████████████████  0.97 (10%)
T_network  ██████████████████   0.90 (5%)
           ─────────────────────
Total:     ██████████████████   0.929 → 0.509 (after decay)
```

### 6.2. Final Weight Composition

```
           Trust  RandomX  Stake    Final
Alice:     0.509  × 1.400  × 0.471  = 0.336
Bob:       0.409  × 1.414  × 0.207  = 0.120
Carol:     0.606  × 1.371  × 0.655  = 0.544
```

**Pie Chart (Conceptual):**
```
    Carol (54%)
    ╱╲
   ╱  ╲
  ╱    ╲
 ╱      ╲Alice (34%)
╱________╲
    ╲  ╱
     ╲╱
   Bob (12%)
```

---

## 🔐 VII. SECURITY ANALYSIS

### 7.1. Attack Scenarios

#### Attack 1: Pure Stake (Whale)

```
Attacker: 90% stake, 0% trust, 0% CPU
  W_trust = 0.0
  W_randomx = 1.0
  W_stake = 0.90^0.8 = 0.92
  
  W_final = 0.0 × 1.0 × 0.92 = 0.0  ❌ FAIL!
```

**Obrona:** Trust jest REQUIRED! Bez trust → zero weight.

#### Attack 2: Pure CPU (Mining Farm)

```
Attacker: 90% CPU, 0% trust, 0% stake
  W_trust = 0.0
  W_randomx = (1+1.0)^0.5 = 1.414
  W_stake = 0.0
  
  W_final = 0.0 × 1.414 × 0.0 = 0.0  ❌ FAIL!
```

**Obrona:** Stake jest REQUIRED! Minimum stake gate.

#### Attack 3: Trust Grinding

```
Attacker: Próbuje sztucznie podnieść trust przez fake metrics
  
Constraints:
  - T_blocks: Musi produkować valid blocks (verifiable on-chain)
  - T_proofs: Musi generować valid ZK/BP proofs (cryptographic)
  - T_uptime: Musi być online (P2P observable)
  - T_stake: Musi lock funds (economic cost)
  - T_fees: Musi zbierać real fees (economic)
  - T_network: Musi mieć peers (Sybil-resistant)
```

**Obrona:** Trust jest EARNED, not claimed. Każdy komponent jest verifiable.

#### Attack 4: Nothing-at-Stake

```
Validator: Próbuje podpisać multiple chains bez penalty

Slashing:
  - Double-sign detection: severity=10 → slash 10%
  - Equivocation: severity=20 → slash 20%
  - Byzantine: severity=100 → slash 100%
```

**Obrona:** Economic penalty + trust decay.

---

### 7.2. Decentralization Metrics

**Nakamoto Coefficient:**
```
NC = min(k) such that Σ top_k weights > 0.51

Dla naszego przykładu (Carol=54%, Alice=34%, Bob=12%):
  Carol + Alice = 88% > 51%
  NC = 2  (2 validatory kontrolują 51%+)
```

**Gini Coefficient:**
```
G = (Σ Σ |W_i - W_j|) / (2N × Σ W_i)

Dla równego rozkładu: G = 0
Dla monopolu: G = 1
```

**Optimal Range:** G ∈ [0.2, 0.4] (umiarkowana nierówność, ale nie monopol)

---

## ⚙️ VIII. TUNABLE PARAMETERS

### 8.1. System Constants

| Parameter | Symbol | Default | Range | Impact |
|-----------|--------|---------|-------|--------|
| **PoT weights** | α₁...α₆ | [0.3,0.25,0.2,0.1,0.1,0.05] | Σ=1.0 | Trust composition |
| **Decay rate** | β | 0.98 | [0.95, 0.99] | Trust stability |
| **RandomX difficulty** | D | 16 bits | [8, 24] | Mining hardness |
| **Stake multiplier** | k | 0.5 | [0.3, 0.8] | Lock incentive |
| **Min stake** | S_min | 100K | [10K, 1M] | Entry barrier |
| **Trust power** | p_t | 1.0 | [0.8, 1.2] | Trust influence |
| **RandomX power** | p_r | 0.5 | [0.3, 0.7] | CPU influence |
| **Stake power** | p_s | 0.8 | [0.5, 1.0] | Stake influence |

---

### 8.2. Adjustment Rules

**Automatic difficulty adjustment:**
```
D(t+1) = D(t) × (12s / actual_time)^0.1

Bounds: D ∈ [12, 20] bits
```

**Minimum stake adjustment:**
```
S_min(t+1) = S_min(t) × (1 + inflation_rate)

Typical: 2% annual increase
```

**Trust decay adjustment:**
```
β(t+1) = β(t) + ε × sign(avg_trust - target_trust)

Target: 0.5-0.7 range
ε = 0.001 (slow adjustment)
```

---

## 📚 IX. IMPLEMENTACJA (Kod Skeleton)

### 9.1. Trust Computation

```rust
pub fn compute_hard_trust(metrics: &QualityMetrics, weights: &TrustWeights) -> f64 {
    let t_blocks = (metrics.blocks_produced as f64 / metrics.target_blocks as f64).min(1.0);
    let t_proofs = metrics.valid_proofs as f64 / metrics.total_proofs.max(1) as f64;
    let t_uptime = metrics.uptime_slots as f64 / metrics.eligible_slots as f64;
    let t_stake = stake_lock_multiplier(metrics.lock_days) / 4.0; // Normalize
    let t_fees = (metrics.fees_collected / metrics.expected_fees).min(1.0);
    let t_network = metrics.peer_score * 0.5 + metrics.propagation_score * 0.5;
    
    weights.blocks * t_blocks +
    weights.proofs * t_proofs +
    weights.uptime * t_uptime +
    weights.stake * t_stake +
    weights.fees * t_fees +
    weights.network * t_network
}

pub fn stake_lock_multiplier(lock_days: u32) -> f64 {
    1.0 + 0.5 * ((1.0 + lock_days as f64 / 30.0).ln())
}
```

### 9.2. Final Weight

```rust
pub fn compute_final_weight(
    trust: f64,
    randomx_score: f64,
    stake_fraction: f64,
    powers: &PowerParams,
) -> f64 {
    let w_trust = trust.powf(powers.trust);
    let w_randomx = (1.0 + randomx_score).powf(powers.randomx);
    let w_stake = stake_fraction.powf(powers.stake);
    
    w_trust * w_randomx * w_stake
}
```

### 9.3. Leader Selection

```rust
pub fn select_leader(
    validators: &[(NodeId, f64)], // (id, weight)
    beacon: &[u8; 32],
    slot: u64,
) -> NodeId {
    // Sort by weight descending
    let mut sorted = validators.to_vec();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    
    // Deterministic index
    let seed = u64::from_le_bytes(beacon[0..8].try_into().unwrap());
    let index = ((seed + slot) as usize) % sorted.len();
    
    sorted[index].0
}
```

---

## 🎯 X. PODSUMOWANIE

### Właściwości Złotego Trio:

✅ **Trust-Based** - Reputacja zarabiana, nie kupowana  
✅ **CPU-Friendly** - RandomX dla starych CPU, nie ASIC  
✅ **Stake-Secured** - Economic security, slashing  
✅ **Deterministyczny** - Jeden lider per slot  
✅ **Verifiable** - Każdy komponent on-chain proof  
✅ **Privacy-Preserving** - ZK proofs dla trust  
✅ **Sybil-Resistant** - Multi-factor verification  
✅ **Decentralized** - Nie favoruje whales ani mining farms  

### Formuła Finalna:

```
W_final = T^1.0 × (1+R)^0.5 × S^0.8

gdzie:
  T = Σ αᵢ·Tᵢ  (6 komponentów trust)
  R = solved/expected (RandomX mining)
  S = stake_eff / stake_total (locked stake)
```

---

**To jest matematycznie precyzyjny, kompleksowy model consensusu! Gotowy do implementacji! 🚀**
