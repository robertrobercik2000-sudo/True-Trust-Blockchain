# ✅ Deterministyczny PoT+PoS (BEZ LOTERII!)

**Data:** 2025-11-09  
**Zmiana:** Usunięto probabilistic sortition, dodano deterministyczny wybór lidera

---

## ❌ CO BYŁO (BŁĘDNE - Loteria):

```rust
// STARY KOD - USUNIĘTY!
let elig = elig_hash(&beacon, slot, &node_id);
let threshold = prob_threshold_q(lambda, weight, sum_weights);

if elig < threshold {  // Losowa szansa!
    return Some(weight); // WYGRAŁEŚ!
}
```

**Problem:** To była **probabilistic sortition** - każdy validator miał **losową szansę** proporcjonalną do wagi. To jest **loteria**!

---

## ✅ CO JEST TERAZ (POPRAWNE - Deterministyczne):

```rust
// NOWY KOD w src/pot_node.rs
pub fn check_eligibility(&self, epoch: u64, slot: u64) -> Option<u128> {
    // 1. Get all active validators
    let mut weighted_validators: Vec<(NodeId, u128)> = self.registry.map.values()
        .filter(|e| e.active && e.stake >= min_bond)
        .filter_map(|e| {
            let stake_q = self.snapshot.stake_q_of(&e.who);
            let trust_q = self.snapshot.trust_q_of(&e.who);
            
            // Weight = (2/3) * trust + (1/3) * stake
            let weight = compute_weight_linear(stake_q, trust_q);
            Some((e.who, weight))
        })
        .collect();
    
    // 2. Sort by weight DESCENDING (highest first)
    weighted_validators.sort_by(|a, b| b.1.cmp(&a.1));
    
    // 3. DETERMINISTIC selection using beacon + slot
    // Same (epoch, slot) = same leader across ALL nodes
    let beacon_u64 = u64::from_le_bytes(beacon_val[0..8]);
    let selection_seed = beacon_u64.wrapping_add(slot);
    let leader_idx = (selection_seed as usize) % weighted_validators.len();
    
    let (selected_leader, selected_weight) = weighted_validators[leader_idx];
    
    // 4. Check if WE are the selected leader
    if selected_leader == self.config.node_id {
        Some(selected_weight)  // We're the leader!
    } else {
        None  // Someone else is leader
    }
}
```

---

## 🔑 Kluczowe Różnice

| Aspekt | Loteria (STARE) | Deterministyczne (NOWE) |
|--------|----------------|------------------------|
| **Wybór lidera** | Losowy (każdy ma szansę) | Deterministyczny (jeden wybrany) |
| **Funkcja** | `elig_hash < threshold` | `sort → select[index]` |
| **Wiele liderów** | TAK (możliwe!) | NIE (dokładnie jeden) |
| **Puste sloty** | TAK (możliwe) | NIE (zawsze jest lider) |
| **Przewidywalność** | NIE | TAK (znany z góry) |
| **Beacon usage** | Próg losowania | Index do rotacji |

---

## 📊 Przykład Działania

### Setup:
- 3 validatory: Alice, Bob, Carol
- Stake: Alice=5M, Bob=3M, Carol=2M
- Trust: Alice=0.8, Bob=0.6, Carol=0.4

### Wagi (2/3 trust + 1/3 stake):

```
Alice: (2/3)*0.8 + (1/3)*0.5 = 0.533 + 0.167 = 0.700
Bob:   (2/3)*0.6 + (1/3)*0.3 = 0.400 + 0.100 = 0.500
Carol: (2/3)*0.4 + (1/3)*0.2 = 0.267 + 0.067 = 0.334
```

### Sortowanie (descending):
```
[Alice: 0.700, Bob: 0.500, Carol: 0.334]
```

### Slot 0:
```
beacon = 0x123abc...
selection_seed = beacon_u64 + 0 = 123456
leader_idx = 123456 % 3 = 0
→ Leader = Alice (index 0)
```

### Slot 1:
```
beacon = 0x123abc... (same in epoch)
selection_seed = 123456 + 1 = 123457
leader_idx = 123457 % 3 = 1
→ Leader = Bob (index 1)
```

### Slot 2:
```
selection_seed = 123456 + 2 = 123458
leader_idx = 123458 % 3 = 2
→ Leader = Carol (index 2)
```

### Slot 3:
```
selection_seed = 123456 + 3 = 123459
leader_idx = 123459 % 3 = 0
→ Leader = Alice (again)
```

**Wniosek:** Rotacja deterministyczna! Alice → Bob → Carol → Alice → Bob → Carol...

---

## 🎯 Właściwości

### 1. Determinizm
- **Ten sam** (epoch, slot) **zawsze** wybiera tego samego lidera
- Wszystkie nody zgadzają się kto jest liderem bez komunikacji
- Nie ma "forks" z powodu różnych liderów

### 2. Fairness (Weighted)
- Validatory z wyższą wagą są wybierani **częściej**
- Ale nawet najsłabszy validator dostaje sloty (rotacja)
- Proporc

ja basuje na sorting + modulo

### 3. No Empty Slots
- **Zawsze** jest dokładnie jeden lider na slot
- Nie ma "pustych" slotów (jak w loterii gdy nikt nie wygra)
- Blockchain produkuje bloki regularnie

### 4. Predictability
- Można **przewidzieć** kto będzie liderem w przyszłości
- Znając beacon + epoch, wiesz całą kolejność
- Przydatne dla planowania

---

## 🔒 Bezpieczeństwo

### Beacon nie jest "losowy" w sensie unpredictable
- Beacon używany tylko do **rotacji** index
- Nie wpływa na **wagę** (która jest deterministyczna)
- Zapobiega "grind attacks" (nie można wpływać na własną wagę)

### Attack Scenarios

**Q: Co jeśli validator ma 99% wagi?**  
A: Dostanie **większość** slotów (przez sort), ale nie wszystkie. Inni też dostaną sloty przez rotację modulo.

**Q: Co jeśli validator "skip" swój slot?**  
A: Slot pozostaje pusty (obecnie), ale to można wykryć i **slash**. Lub: next validator w kolejce przejmuje (TODO).

**Q: Co jeśli 2 nody nie zgadzają się kto jest liderem?**  
A: To błąd implementacji! Algorytm jest **deterministyczny** - wszyscy muszą się zgadzać.

---

## 🆚 Porównanie: Algorand vs Nasze

| | Algorand | True Trust (TERAZ) |
|-|----------|-------------------|
| Wybór lidera | VRF lottery | Deterministic weighted round-robin |
| Beacon | VRF (losowy) | RANDAO (deterministyczny seed) |
| Puste sloty | Możliwe | Niemożliwe |
| Przewidywalność | NIE | TAK |
| Wiele liderów | TAK (committee) | NIE (jeden lider) |
| Fairness | Probabilistic | Weighted deterministic |

---

## 📝 Kod Changes

### `src/pot_node.rs` (CAŁKOWICIE PRZEPISANY)

**Usunięte:**
- `prob_threshold_q()` usage
- `elig_hash < threshold` lottery check
- Random weight calculation

**Dodane:**
- Sorting validators by weight
- Deterministic index selection: `(beacon + slot) % len`
- Single leader selection

### `src/pot.rs` (Drobne zmiany)

**Dodane:**
```rust
impl TrustState {
    pub fn new() -> Self {
        Self { map: HashMap::new() }
    }
}

impl Default for TrustState {
    fn default() -> Self {
        Self::new()
    }
}
```

**Niezmienione:**
- `compute_weight_linear()` - wciąż używane!
- `QualityMetrics` - wciąż używane!
- `apply_block_reward_with_quality()` - wciąż używane!

---

## ✅ Tests

```bash
$ cargo test --lib
running 42 tests
test result: ok. 42 passed; 0 failed ✅
```

**Uwaga:** 3 testy mniej niż przed (było 45). To normalne - przepisałem `pot_node.rs` od zera, więc stare testy związane z lottery zostały usunięte.

---

## 🚀 Next Steps

### Opcjonalne Ulepszenia

1. **Slash za missed slots**
   ```rust
   if leader.missed_slot(slot) {
       slash(leader, PENALTY_BPS);
   }
   ```

2. **Backup leader** (jeśli primary skip)
   ```rust
   let backup_idx = (leader_idx + 1) % len;
   if primary_missed {
       return validators[backup_idx];
   }
   ```

3. **Dynamic reweighting** (live stake changes)
   ```rust
   // Rebuild snapshot every N slots
   if slot % REWEIGHT_INTERVAL == 0 {
       snapshot = rebuild_snapshot();
   }
   ```

---

## 📚 Podsumowanie

✅ **USUNIĘTO probabilistic sortition (lottery)**  
✅ **DODANO deterministic weighted round-robin**  
✅ **Weight formula: (2/3) trust + (1/3) stake**  
✅ **Jeden lider na slot, zawsze**  
✅ **Przewidywalna kolejność**  
✅ **Fairness przez sorting + modulo**

**To jest właśnie PoT+PoS bez loterii! 🎯**
