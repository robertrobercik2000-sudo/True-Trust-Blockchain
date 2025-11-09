# Strategia połączenia gałęzi

## Analiza sytuacji

### Branch aktualny: `cursor/implement-crypto-wallet-cli-6024`
- Ostatni commit: "Refactor: Migrate hashing to KMAC256 for improved security"
- Zawiera: PoT consensus module, KMAC256, snapshot verification
- Lokalne pliki: `src/pot.rs`, `src/snapshot.rs`, `src/crypto_kmac_consensus.rs`

### Branch docelowy: `cursor/implement-crypto-wallet-cli-f1b7`
- Ostatni commit: "docs: Update Known Limitations with current status"
- Zawiera: 
  - PoT consensus module (podobny)
  - KMAC256 implementation
  - Falcon-512 signatures
  - ML-KEM integration
  - Quantum wallet features
  - Więcej dokumentacji i testów

## Różnice kluczowe

### Pliki które mogą kolidować:
1. `src/consensus.rs` vs `src/pot.rs` - podobna funkcjonalność
2. `src/snapshot.rs` - oba mają implementację
3. `src/crypto_kmac_consensus.rs` - oba mają KMAC256
4. `Cargo.toml` - różne zależności

### Pliki tylko w f1b7:
- `falcon_seeded/` - Falcon signatures
- `guests/` - ZKVM guest code
- `pot80_zk_host/` - host code dla ZK
- Więcej dokumentacji

## Strategia merge

### Opcja 1: Merge f1b7 do 6024 (zalecane)
```bash
# 1. Zapisz lokalne zmiany
git stash

# 2. Przełącz się na f1b7
git checkout -b cursor/implement-crypto-wallet-cli-f1b7 origin/cursor/implement-crypto-wallet-cli-f1b7

# 3. Merge naszych zmian
git merge cursor/implement-crypto-wallet-cli-6024

# 4. Rozwiąż konflikty ręcznie
```

### Opcja 2: Rebase 6024 na f1b7
```bash
# 1. Przełącz się na nasz branch
git checkout cursor/implement-crypto-wallet-cli-6024

# 2. Rebase na f1b7
git rebase origin/cursor/implement-crypto-wallet-cli-f1b7

# 3. Rozwiąż konflikty
```

### Opcja 3: Cherry-pick wybrane commity
```bash
# Wybierz tylko potrzebne commity z 6024
git cherry-pick <commit-hash>
```

## Rekomendacja

**Opcja 1 (Merge)** jest najlepsza, bo:
- Zachowuje historię obu branchy
- Łatwiejsze do debugowania
- Możemy wybrać najlepsze części z obu

## Plan działania

1. **Sprawdź konflikty** przed merge
2. **Zapisz lokalne zmiany** (stash)
3. **Wykonaj merge**
4. **Rozwiąż konflikty** (jeśli są)
5. **Przetestuj** połączenie
6. **Commit** wynikowy merge

## Potencjalne konflikty

### 1. `src/consensus.rs` vs `src/pot.rs`
- **Rozwiązanie**: Sprawdź czy są identyczne, jeśli nie - połącz najlepsze części

### 2. `src/snapshot.rs`
- **Rozwiązanie**: Porównaj implementacje, użyj bardziej kompletnej

### 3. `Cargo.toml`
- **Rozwiązanie**: Połącz zależności z obu branchy

### 4. `src/crypto_kmac_consensus.rs`
- **Rozwiązanie**: Sprawdź czy implementacje są zgodne

## Następne kroki

1. Czy chcesz żebym wykonał merge automatycznie?
2. Czy wolisz najpierw zobaczyć szczegółowe różnice?
3. Czy mamy zachować obie wersje plików (np. consensus.rs i pot.rs)?
