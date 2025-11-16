# Code Review: TRUE_TRUST Wallet CLI v4

## Ogólna ocena: ⭐⭐⭐⭐ (4/5)

Kod jest dobrze napisany, bezpieczny i funkcjonalny. Poniżej szczegółowa analiza.

## ✅ Mocne strony

1. **Bezpieczeństwo**
   - `#[forbid(unsafe_code)]` - świetnie!
   - Użycie `zeroize` dla wrażliwych danych
   - Atomic file operations
   - Pepper-based KDF enhancement
   - Proper nonce handling

2. **Architektura**
   - Czytelna struktura modułowa
   - Dobre użycie traitów (`PepperProvider`)
   - Sensowne separacje odpowiedzialności

3. **Funkcjonalność**
   - Kompleksowy zestaw funkcji
   - Shamir secret sharing
   - Wiele opcji AEAD/KDF
   - Skanowanie transakcji

## ⚠️ Potencjalne problemy

### 1. **Błąd w `atomic_replace`** (linia ~450)
```rust
match fs::rename(&tmp, path) {
    Err(_) => {
        let _ = fs::remove_file(path);  // ⚠️ NIEBEZPIECZNE!
        fs::rename(&tmp, path)?;
```
**Problem**: Jeśli `rename` się nie powiedzie, próbujesz usunąć oryginalny plik. To może prowadzić do utraty danych.

**Sugestia**: Najpierw sprawdź czy plik istnieje i czy rename rzeczywiście się nie powiódł z powodu istniejącego pliku.

### 2. **Duplikacja kodu**
Funkcje `cmd_wallet_init` i `create_encrypted_wallet_from_master` mają dużo zduplikowanego kodu. Warto wyekstrahować wspólną logikę.

### 3. **Brak walidacji w `shards_recover`**
```rust
ensure!(paths.len()>=2, "need at least 2 shards");
```
Powinno być: `ensure!(paths.len() >= m as usize, ...)` gdzie `m` pochodzi z pierwszego shardu.

### 4. **Error handling w `shards_recover`**
```rust
let secret = sharks.recover(shares_iter)?;
```
Brak informacji o tym, które shardy były użyte w przypadku błędu.

### 5. **Polskie komentarze w kodzie**
```rust
wallet_id: [u8; 16], // losowe ID portfela do powiązania peppera i shardów
```
Dla międzynarodowego projektu lepiej użyć angielskiego.

### 6. **Hardcoded wartości**
```rust
let mem_kib: u32 = 512 * 1024; // 512 MiB baseline
let time_cost: u32 = 3;
```
Warto uczynić je konfigurowalnymi lub przynajmniej stałymi.

### 7. **Potencjalny problem z `fsync_parent_dir`**
Na Windows `sync_all()` może nie działać jak oczekiwano. Warto dodać komentarz.

## 🔧 Sugestie ulepszeń

### 1. **Refaktoryzacja duplikacji**
```rust
fn create_wallet_header(
    use_argon2: bool,
    aead_flag: AeadFlag,
    pepper_flag: PepperFlag,
    pad_block: u16,
    wallet_id: Option<[u8;16]>,
) -> Result<WalletHeader> {
    // wspólna logika tworzenia headera
}
```

### 2. **Lepsze error messages**
```rust
ensure!(
    paths.len() >= m as usize,
    "need at least {} shards, got {}",
    m,
    paths.len()
);
```

### 3. **Dodaj walidację dla `m` i `n`**
```rust
ensure!(m >= 2 && m <= n && n <= 255, "invalid m-of-n scheme");
```

### 4. **Dodaj testy jednostkowe**
Szczególnie dla:
- `pad`/`unpad`
- `shard_mask`/`shard_unmask`
- `derive_kdf_key`

### 5. **Dokumentacja**
Dodaj `///` doc comments dla publicznych funkcji i struktur.

## 📦 Brakujące zależności

Kod wymaga `Cargo.toml` z następującymi zależnościami:
- `anyhow`
- `clap` z features `derive`
- `rand` z features `std`
- `rpassword`
- `serde` z features `derive`
- `zeroize`
- `aes-gcm`
- `aes-gcm-siv`
- `chacha20poly1305`
- `ed25519-dalek`
- `x25519-dalek`
- `argon2`
- `dirs`
- `bech32`
- `hex`
- `bincode`
- `sharks`
- `pot80_zk_host` (wymaga implementacji lub jest to zewnętrzna zależność)

## 🐛 Potencjalne błędy

1. **Race condition w `OsLocalPepper::get`**
   Jeśli dwa procesy jednocześnie próbują utworzyć pepper, może dojść do race condition. Warto użyć `create_new(true)` zamiast sprawdzania `exists()`.

2. **Brak walidacji rozmiaru `enc_hint`**
   W `cmd_keysearch_pairs` sprawdzasz `MAX_ENC_HINT_BYTES`, ale nie ma walidacji przy tworzeniu w `cmd_build_enc_hint`.

## 💡 Dodatkowe sugestie

1. **Dodaj `--dry-run` flag** dla operacji które modyfikują pliki
2. **Dodaj progress bar** dla długotrwałych operacji (np. Argon2)
3. **Dodaj `--verbose` flag** dla debugowania
4. **Rozważ użycie `secrecy` crate** zamiast `Zeroizing` dla niektórych typów
5. **Dodaj integracyjne testy** dla całego flow wallet init → export → recover

## 📝 Podsumowanie

Kod jest wysokiej jakości i gotowy do użycia po naprawieniu kilku drobnych problemów. Główne obszary do poprawy:
1. Napraw `atomic_replace` 
2. Usuń duplikację kodu
3. Dodaj lepsze error handling
4. Dodaj testy
5. Stwórz `Cargo.toml`

Ogólnie: **Świetna robota!** 👏
