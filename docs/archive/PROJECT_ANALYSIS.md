# Analiza projektu True-Trust-Blockchain

## Struktura projektu

### Repozytorium GitHub:
- **Nazwa**: `quantum_falcon_wallet`
- **Branch**: `cursor/implement-crypto-wallet-cli-f1b7`
- **Główne komponenty**:
  - `src/consensus.rs` - moduł consensus (używa **SHA256**)
  - `src/snapshot.rs` - moduł snapshot (używa **SHA256**)
  - `guests/priv_guest/` - RISC Zero ZKVM guest code (używa **KMAC256**)
  - `pot80_zk_host/` - host code dla ZK proofs
  - `src/tt_priv_cli.rs` - CLI dla wallet

### Nasze lokalne pliki:
- `src/pot.rs` - moduł consensus (używa **SHA3-512** ✅)
- `src/snapshot.rs` - moduł snapshot (używa **SHA3-512** ✅)
- `src/lib.rs` - biblioteka z re-eksportami

## Różnice

### 1. **Nazwy plików**
- **GitHub**: `src/consensus.rs`
- **Lokalnie**: `src/pot.rs`

### 2. **Hash functions**
- **GitHub**: `sha2::{Digest, Sha256}`
- **Lokalnie**: `sha3::{Digest, Sha3_512}` ✅

### 3. **Funkcjonalność**
Oba implementują ten sam moduł Proof-of-Trust, ale:
- GitHub używa SHA256
- Nasze pliki używają SHA3-512 (zgodnie z wymaganiem)

## Co zrobić?

### Opcja 1: Zaktualizować kod w repozytorium
Zmienić `src/consensus.rs` i `src/snapshot.rs` w repozytorium z SHA256 na SHA3-512.

### Opcja 2: Zastąpić lokalne pliki
Użyć kodu z repozytorium jako bazę i zmienić tylko hash functions.

### Opcja 3: Zachować obie wersje
- `consensus.rs` (SHA256) - dla kompatybilności
- `pot.rs` (SHA3-512) - nowa wersja

## Rekomendacja

**Zaktualizować kod w repozytorium** (`src/consensus.rs` i `src/snapshot.rs`) aby używały SHA3-512 zamiast SHA256, ponieważ:
1. Chcesz używać tylko SHA3-512
2. Kod w repozytorium jest źródłem prawdy
3. Zachowamy spójność w całym projekcie

## Pliki do zaktualizowania w repozytorium:

1. **`src/consensus.rs`**:
   - Zmienić `use sha2::{Digest, Sha256}` → `use sha3::{Digest, Sha3_512}`
   - Zaktualizować wszystkie funkcje hashujące (użyć pierwszych 32 bajtów z SHA3-512)

2. **`src/snapshot.rs`**:
   - Zmienić `use sha2::{Digest, Sha256}` → `use sha3::{Digest, Sha3_512}`
   - Zaktualizować `merkle_leaf_hash()` i `merkle_parent()`

3. **`Cargo.toml`**:
   - Upewnić się że `sha3 = "0.10"` jest w zależnościach (już jest ✅)

## Uwaga

Kod guest (`guests/priv_guest/src/main.rs`) **zostaje z KMAC256** - to jest poprawne, bo:
- KMAC to MAC z kluczem (lepsze bezpieczeństwo)
- Używany w kontekście ZKVM
- Różne narzędzie dla różnych celów

## Podsumowanie

✅ **Host code** (consensus, snapshot): SHA3-512
✅ **Guest code** (ZKVM): KMAC256
✅ **Cargo.toml**: ma już `sha3 = "0.10"`

**Następny krok**: Zaktualizować `src/consensus.rs` i `src/snapshot.rs` w repozytorium GitHub aby używały SHA3-512.
