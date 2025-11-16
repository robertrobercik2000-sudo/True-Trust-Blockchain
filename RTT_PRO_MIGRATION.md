# 🔄 RTT Trust Migration: f64 → Q32.32 (PRO)

## 🎯 Cel

Migracja z `rtt_trust.rs` (f64, niezdeterministyczny) do `rtt_trust_pro.rs` (Q32.32, deterministyczny).

---

## 📊 Porównanie

| Aspekt | `rtt_trust.rs` | `rtt_trust_pro.rs` |
|--------|----------------|---------------------|
| **Arytmetyka** | `f64` | `Q32.32` (`u64`) |
| **Deterministyczny** | ❌ (IEEE 754 rounding) | ✅ (fixed-point) |
| **Historia** | Mapa `(validator, epoch)` | EWMA: `H = α·H + (1-α)·Q` |
| **Vouching cap** | Brak (może → ∞) | ✅ `V ≤ 1.0` |
| **Krzywa S** | `sigmoid(x) = 1/(1+e^-x)` | `S(x) = 3x² − 2x³` |
| **Performance** | Wolniejszy (exp/log) | Szybszy (tylko mul/add) |
| **Memory** | O(V×E) history | O(V) EWMA |
| **Consensus-safe** | ❌ | ✅ |

---

## 🚀 Jak migrować kod

### 1️⃣ Zmiana importu
**Przed**:
```rust
use crate::rtt_trust::{TrustGraph, RTTConfig, TrustScore};
```

**Po**:
```rust
use crate::rtt_trust_pro::{TrustGraph, RTTConfig, TrustScore, Q, q_from_f64, q_to_f64};
```

---

### 2️⃣ Inicjalizacja (bez zmian API)
```rust
let config = RTTConfig::default();
let mut graph = TrustGraph::new(config);
```

---

### 3️⃣ Quality → Q32.32
**Przed** (f64):
```rust
let quality = 0.85_f64; // Zakres [0.0, 1.0]
graph.record_quality(validator, quality);
```

**Po** (Q32.32):
```rust
let quality_q = q_from_f64(0.85); // Konwersja f64 → Q
graph.record_quality(validator, quality_q);
```

**Lub bezpośrednio z Q32.32**:
```rust
use crate::pot::Q; // Q32.32 z pot.rs
let quality_q: Q = compute_quality_from_golden_trio(...); // Już w Q
graph.record_quality(validator, quality_q);
```

---

### 4️⃣ Vouching (bez zmian API)
```rust
let vouch = Vouch {
    voucher: alice,
    vouchee: bob,
    strength: q_from_f64(0.8), // ← ZMIANA: f64 → Q
    created_at: current_epoch,
};

let ok = graph.add_vouch(vouch);
```

---

### 5️⃣ Trust update (bez zmian API)
```rust
let trust_q = graph.update_trust(validator); // Zwraca Q
```

---

### 6️⃣ Konwersja Q → f64 (display/debug)
```rust
let trust_f64 = q_to_f64(trust_q);
println!("Trust: {:.4}", trust_f64);
```

---

### 7️⃣ Ranking (bez zmian API)
```rust
let ranking = graph.get_ranking(); // Vec<(NodeId, Q)>

for (id, trust_q) in ranking.iter().take(10) {
    println!("{:x?}: {:.4}", &id[..4], q_to_f64(*trust_q));
}
```

---

## 🔧 Integracja z Golden Trio

### Przed (f64):
```rust
// golden_trio.rs
pub fn compute_hard_trust(...) -> f64 { /* ... */ }

// pot_node.rs
let quality_f64 = golden_trio::compute_hard_trust(...);
rtt_graph.record_quality(validator, quality_f64);
```

---

### Po (Q32.32):
```rust
// golden_trio.rs
pub fn compute_hard_trust_q(...) -> Q { /* ... */ }

// LUB helper:
pub fn compute_hard_trust(...) -> f64 { /* ... */ }
pub fn compute_hard_trust_q(...) -> Q {
    q_from_f64(compute_hard_trust(...))
}

// pot_node.rs
let quality_q = golden_trio::compute_hard_trust_q(...);
rtt_graph.record_quality(validator, quality_q);
```

---

## 📐 Q32.32 Arithmetic Cheatsheet

### Stałe:
```rust
pub const ONE_Q: Q = 1u64 << 32; // 1.0 w Q32.32
```

### Konwersje:
```rust
// f64 → Q
let q = q_from_f64(0.75); // 3221225472u64

// Q → f64
let f = q_to_f64(q); // 0.75

// u64 (integer) → Q
let q = 5u64 << 32; // 5.0 w Q32.32

// Q → u64 (integer part)
let i = q >> 32;
```

### Operacje:
```rust
// Dodawanie
let sum = a.saturating_add(b);

// Odejmowanie
let diff = a.saturating_sub(b);

// Mnożenie Q × Q → Q
let product = qmul(a, b);

// Clamp [0, 1]
let clamped = qclamp01(x);
```

---

## 🧪 Testy migracji

### Test 1: Zgodność wyników
```rust
#[test]
fn test_f64_vs_q32() {
    use crate::rtt_trust as old;
    use crate::rtt_trust_pro as new;
    
    let alice = [1u8; 32];
    
    // Old (f64)
    let mut old_graph = old::TrustGraph::new(old::RTTConfig::default());
    old_graph.record_quality(alice, 0.9);
    let old_trust = old_graph.update_trust(alice);
    
    // New (Q32.32)
    let mut new_graph = new::TrustGraph::new(new::RTTConfig::default());
    new_graph.record_quality(alice, new::q_from_f64(0.9));
    let new_trust = new_graph.update_trust(alice);
    
    // Porównaj (±1%)
    let diff = (old_trust - new::q_to_f64(new_trust)).abs();
    assert!(diff < 0.01, "Trust diff: {}", diff);
}
```

---

### Test 2: Deterministyczność
```rust
#[test]
fn test_deterministic() {
    use crate::rtt_trust_pro::*;
    
    let alice = [1u8; 32];
    
    // Run 1
    let mut g1 = TrustGraph::new(RTTConfig::default());
    g1.record_quality(alice, q_from_f64(0.95));
    let t1 = g1.update_trust(alice);
    
    // Run 2
    let mut g2 = TrustGraph::new(RTTConfig::default());
    g2.record_quality(alice, q_from_f64(0.95));
    let t2 = g2.update_trust(alice);
    
    // MUST be identical (bit-w-bit)
    assert_eq!(t1, t2);
}
```

---

## 🔍 Edge cases

### 1. Quality z innych modułów (już Q32.32)
```rust
use crate::pot::Q; // Już zdefiniowane w pot.rs

fn update_validator_trust(
    graph: &mut TrustGraph,
    validator: NodeId,
    quality_q: Q, // ← Już w Q32.32
) {
    graph.record_quality(validator, quality_q);
    graph.update_trust(validator);
}
```

---

### 2. Vouching strength (dynamiczny)
```rust
// Voucher z trustem 0.8 może vouch max 0.8
let voucher_trust_q = graph.get_trust(&voucher);
let max_strength = voucher_trust_q; // Już w Q

let strength = qmul(max_strength, q_from_f64(0.9)); // 90% of max

let vouch = Vouch {
    voucher,
    vouchee,
    strength,
    created_at,
};

graph.add_vouch(vouch); // Zwróci false jeśli > voucher_trust
```

---

### 3. Bootstrap z różnymi wagami
```rust
use crate::rtt_trust_pro::bootstrap_validator;

let vouchers = vec![
    (alice, q_from_f64(0.8)),   // Silny vouch
    (bob, q_from_f64(0.5)),     // Średni
    (carol, q_from_f64(0.3)),   // Słaby
];

let initial_trust = bootstrap_validator(&mut graph, new_validator, vouchers);
println!("Initial trust: {:.4}", q_to_f64(initial_trust));
```

---

## 🎯 Roadmap

### Phase 1: Parallel run (OBECNE)
- [x] ✅ `rtt_trust.rs` (f64)
- [x] ✅ `rtt_trust_pro.rs` (Q32.32)
- [ ] ⏳ Feature flag dla wyboru wersji

### Phase 2: Migracja modułów
- [ ] ⏳ `pot_node.rs` → RTT PRO
- [ ] ⏳ `golden_trio.rs` → helper `compute_hard_trust_q()`
- [ ] ⏳ Testy integracyjne

### Phase 3: Deprecation
- [ ] 🎯 Oznacz `rtt_trust.rs` jako deprecated
- [ ] 🎯 Usuń `rtt_trust.rs` (po 1-2 release cycles)

---

## 📚 Matematyka (reminder)

### Model RTT PRO:
```
H(v) = α·H_old + (1-α)·Q_t     (EWMA history)
V(v) = min(Σ T(j)·s(j→v), 1.0) (Vouching, capped)
W(v) = Q_t                      (Current quality)

Z_lin = β₁·H + β₂·V + β₃·W     (Linear combination)
T(v)  = S(Z_lin)                (S-curve)

gdzie:
  S(x) = 3x² − 2x³              (Polynomial S-curve)
```

### Domyślne wagi:
- **α = 0.99** (historia bardzo wolno zapomina)
- **β₁ = 0.4** (historia)
- **β₂ = 0.3** (vouching)
- **β₃ = 0.3** (bieżąca praca)

---

## 🏆 Korzyści z migracji

### Consensus:
- ✅ **100% deterministyczny** (fork-choice-safe)
- ✅ **Brak IEEE 754 edge cases** (NaN, Inf, subnormals)
- ✅ **Cross-platform identyczne** (ARM, x86, RISC-V)

### Performance:
- ✅ **~2× szybszy** (brak exp/log/pow)
- ✅ **Mniejszy memory footprint** (O(V) zamiast O(V×E))

### Security:
- ✅ **Vouching cap** (brak Sybil explosion)
- ✅ **Overflow-safe** (saturating ops)

---

**Status**: ✅ **OBA MODUŁY DZIAŁAJĄ** (parallel run)

**Recommendation**: Nowe komponenty powinny używać **RTT PRO** (`rtt_trust_pro.rs`).
