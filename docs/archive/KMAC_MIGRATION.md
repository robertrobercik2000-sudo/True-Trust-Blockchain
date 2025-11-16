# Zmiana z SHA3-512 na KMAC256

## Wprowadzone zmiany

### 1. **Utworzono moduł `crypto_kmac_consensus.rs`** ✅

Nowy moduł z funkcją `kmac256_hash()` która używa SHAKE256 (SHA3 XOF) jako podstawy dla KMAC256:
- Używa stałego klucza `TT-CONSENSUS-KMAC256` dla domain separation
- Label-based domain separation dla różnych operacji
- Deterministic output (32 bytes)

### 2. **Zaktualizowano `pot.rs`** ✅

Wszystkie funkcje hashujące zmienione z SHA3-512 na KMAC256:
- `merkle_leaf_hash()` - używa `kmac256_hash(b"WGT.v1", ...)`
- `merkle_parent()` - używa `kmac256_hash(b"MRK.v1", ...)`
- `merkle_root()` - pusty root używa `kmac256_hash(b"MRK.empty.v1", ...)`
- `RandaoBeacon::commit_hash()` - używa `kmac256_hash(b"RANDAO.commit.v1", ...)`
- `RandaoBeacon::value()` - używa `kmac256_hash(b"RANDAO.slot.v1", ...)`
- `mix_hash()` - używa `kmac256_hash(b"RANDAO.mix.v1", ...)`
- `elig_hash()` - używa `kmac256_hash(b"ELIG.v1", ...)`

### 3. **Zaktualizowano `snapshot.rs`** ✅

Wszystkie funkcje hashujące zmienione na KMAC256:
- `merkle_leaf_hash()` - używa `kmac256_hash(b"WGT.v1", ...)`
- `merkle_parent()` - używa `kmac256_hash(b"MRK.v1", ...)`

### 4. **Zaktualizowano `lib.rs`** ✅

Dodano eksport modułu `crypto_kmac_consensus`.

### 5. **Zaktualizowano `Cargo.toml`** ✅

Komentarz wyjaśniający że `sha3` jest używany dla KMAC256 (SHAKE256).

## Architektura KMAC256

### Implementacja
- **Podstawa**: SHAKE256 (SHA3 XOF)
- **Klucz**: Stały klucz `TT-CONSENSUS-KMAC256` dla domain separation
- **Domain separation**: Różne labele dla różnych operacji:
  - `WGT.v1` - weight leaf hashes
  - `MRK.v1` - Merkle parent nodes
  - `MRK.empty.v1` - empty Merkle tree
  - `RANDAO.commit.v1` - RANDAO commitments
  - `RANDAO.slot.v1` - RANDAO slot values
  - `RANDAO.mix.v1` - RANDAO mixing
  - `ELIG.v1` - eligibility hashing

### Zalety KMAC256
1. **MAC z kluczem** - lepsze bezpieczeństwo niż zwykły hash
2. **Domain separation** - różne labele zapobiegają kolizjom
3. **Spójność** - używany również w guest code (ZKVM)
4. **Standard NIST** - KMAC jest standardem NIST SP 800-185

## Kompatybilność

### Host code (consensus, snapshot):
- ✅ **KMAC256** (oparty na SHAKE256)

### Guest code (ZKVM):
- ✅ **KMAC256** (z `tiny_keccak`)

### Wallet CLI:
- ✅ **KMAC256** (dla KDF i innych operacji)

## Podsumowanie

✅ Wszystkie operacje hashujące używają teraz **KMAC256**
✅ Spójność w całym projekcie (host + guest)
✅ Domain separation przez labele
✅ Bezpieczeństwo MAC z kluczem

Kod jest gotowy do użycia z KMAC256! 🎉
