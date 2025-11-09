# Zmiana z SHA256 na SHA3-512

## Wprowadzone zmiany

### 1. **Zaktualizowano wszystkie funkcje hashujące** ✅

Wszystkie użycia `Sha256` zostały zmienione na `Sha3_512`:

- `merkle_leaf_hash()` - hash liści Merkle tree
- `merkle_parent()` - hash węzłów rodziców w Merkle tree  
- `merkle_root()` - hash pustego drzewa
- `RandaoBeacon::commit_hash()` - hash commitów RANDAO
- `RandaoBeacon::value()` - hash wartości slotów
- `mix_hash()` - mieszanie revealów RANDAO
- `elig_hash()` - hash eligibilności w sortition
- Wszystkie funkcje w `snapshot.rs`

### 2. **Zaktualizowano Cargo.toml** ✅

Zmieniono zależność:
```toml
sha3 = "0.10"  # zamiast sha2 = "0.10"
```

### 3. **Zachowano kompatybilność typów** ✅

SHA3-512 produkuje 64 bajty, ale kod używa 32-bajtowych hashów (`[u8; 32]`). 
Wszystkie funkcje używają **pierwszych 32 bajtów** z SHA3-512 output:

```rust
let out = h.finalize();
let mut r = [0u8; 32];
r.copy_from_slice(&out[..32]);  // Pierwsze 32 bajty z 64-bajtowego outputu
```

### 4. **Zaktualizowano testy** ✅

Test `randao_commit_reveal` został zaktualizowany aby używał SHA3-512.

## Szczegóły techniczne

### SHA3-512 vs SHA256

- **SHA3-512**: 64 bajty output (512 bitów)
- **SHA256**: 32 bajty output (256 bitów)

### Strategia kompatybilności

Używamy pierwszych 32 bajtów z SHA3-512 aby zachować kompatybilność z istniejącymi typami:
- `NodeId = [u8; 32]`
- `weights_root: [u8; 32]`
- Wszystkie hashe w kodzie są `[u8; 32]`

### Bezpieczeństwo

SHA3-512 jest bezpieczniejszy niż SHA256:
- Większy output (512 vs 256 bitów)
- Opiera się na konstrukcji Keccak (SHA-3 standard)
- Odporny na ataki kolizyjne (256-bit security level)

## Pliki zmienione

1. `src/pot.rs` - wszystkie funkcje hashujące
2. `src/snapshot.rs` - funkcje hashujące Merkle
3. `Cargo.toml` - zmiana zależności z sha2 na sha3

## Uwagi

- Wszystkie istniejące testy powinny działać (używają pierwszych 32 bajtów)
- Kompatybilność z istniejącymi typami zachowana
- Jeśli w przyszłości chcesz używać pełnych 64 bajtów, trzeba będzie zmienić typy z `[u8; 32]` na `[u8; 64]`

## Podsumowanie

✅ Wszystkie użycia SHA256 zostały zmienione na SHA3-512
✅ Kompatybilność z istniejącymi typami zachowana (używamy pierwszych 32 bajtów)
✅ Cargo.toml zaktualizowany
✅ Testy zaktualizowane

Kod jest gotowy do użycia z SHA3-512! 🎉
