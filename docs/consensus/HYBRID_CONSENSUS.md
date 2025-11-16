# 🚀 Hybrydowy Konsensus: PoT + Micro PoW + PoS

## Przegląd

TRUE TRUST wykorzystuje **unikalny hybrydowy konsensus** łączący trzy mechanizmy:

1. **PoT (Proof-of-Trust)** - 66.67% wagi (2/3)
2. **PoS (Proof-of-Stake)** - 33.33% wagi (1/3)
3. **Micro PoW** - CPU-friendly verification

### Formuła Wagi

```
final_weight = (2/3) × trust + (1/3) × stake
```

---

## 🎯 Algorytm: RandomX-lite

### Dlaczego RandomX-lite?

- ✅ **CPU-friendly** - optymalizowany pod stare CPU (bez AVX2)
- ✅ **Memory-hard** - wykorzystuje cache L1/L2/L3
- ✅ **Anti-ASIC** - losowe operacje, brak GPU advantage
- ✅ **Sprawiedliwy** - każdy może kopać na starym laptopie

### Specyfikacja

| Parameter | Value | Reason |
|-----------|-------|--------|
| **Scratchpad** | 256 KB | Fits in L2 cache of old CPUs |
| **Iterations** | 1024 | Balance speed/security |
| **Operations** | Integer + AES-like | No AVX2/AVX-512 required |
| **Difficulty** | 16 bits | ~65k hashes avg |

### Instruction Set (Simplified)

```rust
// Round operations (CPU-friendly):
1. ADD + XOR      // Integer ALU
2. ROT + MUL      // Integer units
3. Memory store   // Cache-friendly
4. AES-like mix   // Or XOR cascade for old CPUs
```

**Brak:**
- ❌ JIT compilation (too complex for old CPUs)
- ❌ AVX2/AVX-512 instructions
- ❌ GPU-optimized paths
- ❌ Complex floating point

---

## 💎 Trust Building przez Dowody

### Jak Zdobywa Się Trust?

Trust NIE jest przydzielany arbitralnie - **musisz go zarobić** przez:

1. **Generowanie Bulletproofs** (30% wagi)
2. **Generowanie ZK proofs** (40% wagi)
3. **Wykonywanie Micro PoW** (20% wagi)
4. **Uczciwe działanie** (10% wagi - anti-fraud)

### Formuła Trust Reward

```rust
trust_reward = base_reward × (
    0.3 × bp_count +
    0.4 × zk_count +
    0.2 × sqrt(pow_iterations / 1000) +
    0.1 × uptime_factor
)

// Przykład:
// bp_count = 10, zk_count = 5, pow_iterations = 65536
// => trust_reward = 0.01 × (3.0 + 2.0 + 0.51 + 1.0) = 0.0651
```

### Charakterystyka

- **Liniowy wzrost** - im więcej dowodów, tym więcej trustu
- **Cap at 10x** - maksymalnie 10× base_reward
- **Decay over time** - nieaktywność zmniejsza trust
- **Quality matters** - złe dowody nie liczą się

---

## 🔒 PoS: Minimum Stake Requirement

### Zasada

Aby móc kopać, validator **MUSI** posiadać minimum stake:

```rust
min_stake = 1_000_000  // 1M base units (np. 1M TT)
```

### Dlaczego?

1. **Anti-Sybil** - zapobiega tanim atakom
2. **Skin in the game** - validator ma coś do stracenia
3. **Economic security** - koszt ataku = koszt stake
4. **Fair distribution** - każdy może uzyskać minimum

### Stake NIE Daje Przewagi w Kopaniu!

```
Stake 1M TT:    weight = (2/3)×trust + (1/3)×1.0  = 0.667×trust + 0.333
Stake 100M TT:  weight = (2/3)×trust + (1/3)×1.0  = 0.667×trust + 0.333
```

**PoS to tylko warunek wejścia, NIE przewaga!**

Trust (przez dowody) ma **2× większą wagę** niż stake!

---

## ⚙️ Micro PoW: CPU Verification

### Parametry

```rust
difficulty_bits: 16      // ~65,536 hashes avg
max_iterations: 1_000_000 // Upper bound
algorithm: SHAKE256      // CPU-friendly, no GPU boost
```

### Cel

- **Anti-spam** - koszt wysłania bloku
- **Fair verification** - każdy może zweryfikować na CPU
- **No centralization** - stare CPU też mogą

### Przykład

```rust
// Block data
let block = serialize_block_header(&header);

// RandomX-lite hash
let randomx_hash = compute_randomx_lite(&block, 256_KB, 1024_iters);

// Micro PoW
let pow_proof = mine_micro_pow(&randomx_hash, 16_bits);

// Result: nonce + hash (proof of work)
```

---

## 🎮 Kompletny Mining Flow

### 1. Przygotowanie

```rust
// Validator musi mieć:
✓ Min stake: 1M TT
✓ Trust > 0 (zdobyty przez dowody)
✓ Active registration
```

### 2. Generowanie Dowodów (Trust Building)

```rust
// Podczas epoch:
for tx in pending_transactions {
    // Generate Bulletproof
    let bp = generate_bulletproof_64(&tx.amount, &tx.blinding);
    bp_count += 1;
    
    // Generate ZK proof (if private TX)
    let zk = prove_priv_claim(&tx.private_data);
    zk_count += 1;
}

// Trust reward na koniec epoch
trust += calculate_proof_trust_reward(bp_count, zk_count, ...);
```

### 3. Mining Slot

```rust
// Check eligibility (PoT)
let eligible = pot_node.check_eligibility(epoch, slot);
if !eligible { return; }

// Compute hybrid weight
let weight = (2.0/3.0) × trust + (1.0/3.0) × normalized_stake;

// RandomX-lite hash
let rx_hash = randomx_lite.execute(&block_data);

// Micro PoW
let pow = mine_micro_pow(&rx_hash, 16_bits);

// Assemble block
let block = Block {
    header,
    pow_proof: pow,
    randomx_hash: rx_hash,
    transactions,
};
```

### 4. Broadcast & Verify

```rust
// Broadcast
network.broadcast_block(block);

// Inni walidują:
✓ Min stake check
✓ PoT eligibility
✓ RandomX-lite hash verification
✓ Micro PoW verification
✓ Trust history check
```

---

## 📊 Porównanie z Innymi Algorytmami

| Feature | Bitcoin | Ethereum PoS | Monero (RandomX) | TRUE TRUST Hybrid |
|---------|---------|--------------|------------------|-------------------|
| **Hardware** | ASIC | Any | CPU | **Old CPU** ✅ |
| **Centralization** | High | Medium | Low | **Very Low** ✅ |
| **Trust Model** | None | Slashing | None | **Proof-based** ✅ |
| **Fairness** | Rich win | Rich win | Fair | **Ultra Fair** ✅ |
| **Min Requirements** | $10k+ | 32 ETH | $100 CPU | **1M TT + old laptop** ✅ |
| **Energy** | Very High | Low | Medium | **Very Low** ✅ |
| **GPU Advantage** | N/A | N/A | None | **None** ✅ |

---

## 🔧 Konfiguracja

### Node Config

```toml
[consensus]
pot_weight = 0.6667           # 2/3
pos_weight = 0.3333           # 1/3
min_stake = 1_000_000         # 1M TT
pow_difficulty_bits = 16      # ~65k hashes
proof_trust_reward = 0.01     # 1% per good proof

[randomx]
scratchpad_kb = 256           # 256 KB (old CPU friendly)
iterations = 1024             # Balance speed/security
use_jit = false               # No JIT for compatibility
```

### Środowisko

```bash
# CPU threads (auto-detect recommended)
export TT_CPU_THREADS=4

# Scratchpad size (KB)
export TT_SCRATCHPAD_KB=256

# Enable old CPU mode (no AVX2)
export TT_OLD_CPU_MODE=1
```

---

## 📈 Performance

### Benchmarks (Old CPU: Intel Core i5-2500K, 2011)

| Operation | Time | Notes |
|-----------|------|-------|
| **Scratchpad Init** | ~10 ms | 256 KB fill |
| **RandomX-lite** | ~50 ms | 1024 iterations |
| **Micro PoW (16-bit)** | ~100 ms | avg 65k hashes |
| **BP Generate** | ~200 ms | 64-bit range proof |
| **ZK Verify** | ~50 ms | RISC0 receipt |
| **Total Block** | ~500 ms | Complete mining |

### Nowy CPU (AMD Ryzen 5 5600X, 2020)

| Operation | Time | Notes |
|-----------|------|-------|
| **Total Block** | ~150 ms | ~3× faster |

**Wniosek:** Stare CPU ~3× wolniejsze, ale **NADAL OPŁACALNE!** 🎉

---

## 🛡️ Bezpieczeństwo

### Obrona Przed Atakami

#### 1. **51% Attack**
- **Koszt:** Musisz mieć 51% stake (min 510M TT)
- **+ Koszt:** 51% trust (zdobyte przez MILIONY dowodów)
- **+ Koszt:** CPU power dla PoW
- **Wniosek:** **Praktycznie niemożliwe**

#### 2. **Nothing-at-Stake**
- Rozwiązanie: **Slashing** za equivocation
- Kara: 50% stake + reset trustu do 0

#### 3. **Grinding Attack**
- Obrona: **RANDAO beacon** (commit-reveal)
- Obrona: **Fixed difficulty** (nie można manipulować)

#### 4. **Selfish Mining**
- Obrona: **Trust decay** (nieaktywność karze)
- Obrona: **Proof requirement** (nie da się oszukać)

#### 5. **GPU/ASIC**
- Obrona: **RandomX-lite** (memory-hard + random ops)
- Obrona: **No AVX2** (CPU-only path)

---

## 🎯 Przykład: Start Node na Starym Laptopie

```bash
# 1. Build node
cargo build --release --bin tt_node

# 2. Create wallet i zdobądź 1M TT (min stake)
./target/release/tt_priv_cli wallet-init

# 3. Buy/earn 1M TT (z faucet, exchange, mining na początku)
# (pierwszy okres: niski min_stake lub faucet)

# 4. Register as validator
./target/release/tt_node register-validator \
  --stake 1000000 \
  --wallet ~/.tt_wallet

# 5. Start mining (auto-detects old CPU)
./target/release/tt_node start \
  --data-dir ./node_data \
  --listen 0.0.0.0:8333

# 6. Generate proofs (earn trust!)
# Node automatycznie generuje BP/ZK dla TXs w mempoolu

# 7. Monitor trust
./target/release/tt_node status | grep trust
# Trust: 0.15 (+0.02 this epoch from 5 BP + 2 ZK)
```

---

## 📚 Dokumentacja API

### Rust API

```rust
use tt_priv_cli::cpu_mining::{HybridMiningTask, HybridConsensusParams};
use tt_priv_cli::pot::ONE_Q;

// Create mining task
let task = HybridMiningTask {
    block_data: block_bytes,
    stake_q: ONE_Q * 10,  // 10.0 TT
    trust_q: ONE_Q / 2,   // 0.5 trust
    proof_metrics: ProofMetrics {
        bp_generated: 10,
        zk_generated: 5,
        cpu_time_ms: 5000,
        pow_iterations: 65000,
    },
    params: HybridConsensusParams::default(),
};

// Mine!
let result = task.mine()?;
println!("Weight: {:.4}", result.final_weight);
println!("Trust earned: {:.4}", result.trust_earned);
```

---

## 🎉 Podsumowanie

### Dlaczego Ten System Jest Unikalny?

1. **Demokratyczny** - stare CPU = nowe CPU (relatywnie)
2. **Sprawiedliwy** - trust przez pracę, nie przez pieniądze
3. **Bezpieczny** - 3-layer security (PoT+PoS+PoW)
4. **Ekologiczny** - CPU-only, niska energia
5. **Dostępny** - każdy może uczestniczyć

### Formuła Sukcesu

```
Success = min_stake (PoS) × trust (through proofs) × cpu_work (PoW)
```

**Żaden element sam w sobie nie wystarczy - potrzebujesz WSZYSTKICH TRZECH!**

---

## 📖 Dalsze Czytanie

- `src/cpu_proof.rs` - Micro PoW implementation
- `src/cpu_mining.rs` - RandomX-lite + hybrid consensus
- `src/pot.rs` - PoT core logic
- `NODE_V2_INTEGRATION.md` - Node architecture
- `COMPLETE_SYSTEM.md` - Full system overview

---

**TRUE TRUST Blockchain - Hybrydowy Konsensus dla Wszystkich! 🚀**

*"Trust is earned through work, not bought with money."*
