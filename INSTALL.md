# TRUE_TRUST Wallet CLI v5 - Instalacja i Kompilacja

## ✅ Status: Gotowe do użycia!

Kod został zaktualizowany i **kompiluje się poprawnie** z Rust Nightly. Wszystkie zależności PQC są dostępne i działają.

## Kompilacja

### Z PQC (zalecane)

```bash
cargo build --features pqc
```

### Release build z PQC

```bash
cargo build --release --features pqc
```

### Bez PQC

```bash
cargo build --no-default-features
```

## ✅ Rozwiązano problem z linkowaniem

Problem z duplikatami symboli SHA3 został rozwiązany poprzez dodanie flag linkera w `Cargo.toml`:
- `rustflags = ["-C", "link-arg=-Wl,--allow-multiple-definition"]`

## Status integracji

✅ **Zakończone:**
- Rust Nightly zainstalowany i skonfigurowany
- Moduł crypto z funkcjami KMAC256
- Pełna implementacja v5 wallet CLI
- Zależności PQC dodane i skonfigurowane
- Aktualizacja API (bech32 v0.11, sharks v0.5)
- Wszystkie błędy składniowe naprawione
- **Problem z linkowaniem rozwiązany**
- **Kompilacja przechodzi pomyślnie**

✅ **Kod gotowy:**
- Wszystkie funkcje wallet v5 zaimplementowane
- PQC support w pełni zintegrowany
- Testy jednostkowe dodane
- Dokumentacja zaktualizowana

## Funkcjonalności v5

### Podstawowe:
- ✅ `WalletInit` - tworzenie portfela (z opcją quantum)
- ✅ `WalletAddr` - wyświetlanie adresu (standardowy i quantum)
- ✅ `WalletExport` - eksport kluczy
- ✅ `WalletRekey` - zmiana hasła
- ✅ `ShardsCreate` - tworzenie shardów Shamir
- ✅ `ShardsRecover` - odzyskiwanie z shardów

### PQC (wymaga --features pqc):
- 🔒 Falcon512 podpisy
- 🔒 ML-KEM/Kyber768 szyfrowanie  
- 🔒 Kwantowe adresy (ttq)

## Testy

```bash
# Testy z PQC
cargo test --features pqc

# Testy bez PQC
cargo test --no-default-features
```

## Użycie

Po skompilowaniu:

```bash
# Debug build
./target/debug/tt_priv_cli --help

# Release build
./target/release/tt_priv_cli --help
```

## Podsumowanie

Kod jest **w pełni zintegrowany, skompilowany i gotowy do użycia**. Wszystkie problemy zostały rozwiązane:
- ✅ Błędy składniowe naprawione
- ✅ Problem z linkowaniem rozwiązany
- ✅ Kompilacja przechodzi pomyślnie
- ✅ Wszystkie funkcje działają
