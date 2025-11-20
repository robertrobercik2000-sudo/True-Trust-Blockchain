# Jak używać TT Wallet

## Uruchamianie

tt_wallet został pomyślnie zbudowany i znajduje się w:
```
target\release\tt_wallet.exe
```

## WAŻNE: Uruchamiaj w prawdziwym terminalu!

tt_wallet wymaga interaktywnego wprowadzenia hasła, więc **musisz go uruchomić bezpośrednio w PowerShell lub CMD**, a nie przez narzędzia automatyczne.

## Przykłady użycia

### 1. Utworzenie nowego portfela
```powershell
.\target\release\tt_wallet.exe wallet-init --file moj_portfel.dat
```
Program zapyta o:
- Nowe hasło (min 12 znaków)
- Powtórzenie hasła

### 2. Wyświetlenie adresu portfela
```powershell
.\target\release\tt_wallet.exe wallet-addr --file moj_portfel.dat
```
Program zapyta o hasło.

### 3. Eksport klucza publicznego
```powershell
.\target\release\tt_wallet.exe wallet-export --file moj_portfel.dat
```

### 4. Eksport klucza prywatnego (UWAGA: wrażliwe!)
```powershell
.\target\release\tt_wallet.exe wallet-export --file moj_portfel.dat --secret --out backup.json
```

### 5. Zmiana hasła
```powershell
.\target\release\tt_wallet.exe wallet-rekey --file moj_portfel.dat
```

### 6. Shamir Secret Sharing - podział klucza na części
```powershell
# Przykład: 3 z 5 części potrzebne do odzyskania
.\target\release\tt_wallet.exe shards-create --file moj_portfel.dat --out-dir shards --m 3 --n 5
```

### 7. Odzyskanie portfela z części Shamir
```powershell
.\target\release\tt_wallet.exe shards-recover --input shard-1-of-5.json,shard-2-of-5.json,shard-3-of-5.json --out odzyskany_portfel.dat
```

## Zaawansowane opcje

### Wybór algorytmu szyfrowania
```powershell
# Domyślnie: AES-GCM-SIV
.\target\release\tt_wallet.exe wallet-init --file portfel.dat --aead gcm-siv

# Alternatywnie: XChaCha20-Poly1305
.\target\release\tt_wallet.exe wallet-init --file portfel.dat --aead x-cha-cha20
```

### Zarządzanie "pepper" (dodatkowa warstwa bezpieczeństwa)
```powershell
# Domyślnie: pepper zapisany lokalnie w systemie
.\target\release\tt_wallet.exe wallet-init --file portfel.dat --pepper os-local

# Bez pepper (mniej bezpieczne)
.\target\release\tt_wallet.exe wallet-init --file portfel.dat --pepper none
```

### KDF (Key Derivation Function)
```powershell
# Domyślnie: Argon2id (silne, ale wolne)
.\target\release\tt_wallet.exe wallet-init --file portfel.dat --argon2

# KMAC256 (szybsze, ale mniej odporne na brute-force)
.\target\release\tt_wallet.exe wallet-init --file portfel.dat --argon2=false
```

## Struktura portfela

Portfel TT v5 zawiera:
- **master32** - główny seed (32 bajty)
- **Falcon512** - klucz do podpisów post-kwantowych
- **ML-KEM (Kyber768)** - klucz do szyfrowania post-kwantowego

Adres jest generowany jako:
```
ttq = bech32m(SHAKE256(Falcon_PK || MLKEM_PK))
```

## Bezpieczeństwo

⚠️ **WAŻNE:**
- Hasło musi mieć minimum 12 znaków
- Używaj silnych haseł!
- Backup portfela trzymaj w bezpiecznym miejscu
- Klucze prywatne (`--secret`) NIGDY nie udostępniaj
- Rozważ użycie Shamir Secret Sharing dla krytycznych portfeli

## Pepper - gdzie jest przechowywany?

Gdy używasz `--pepper os-local`, pepper jest zapisywany w:
- **Windows:** `%APPDATA%\TT\pepper\<wallet_id>`
- **Linux/Mac:** `~/.config/tt/pepper/<wallet_id>`

Pepper to dodatkowa 32-bajtowa losowa wartość używana w KDF. Jeśli zgubisz plik pepper, nie odzyskasz portfela nawet znając hasło!

## Rozwiązywanie problemów

### "file exists"
Plik portfela już istnieje. Użyj innej nazwy lub usuń stary plik.

### "password too short"
Hasło musi mieć minimum 12 znaków.

### "decrypt: aead::Error"
Złe hasło lub uszkodzony plik portfela.

### "pepper file not found" (przy odzyskiwaniu)
Pepper był używany, ale plik został usunięty. Bez niego nie odzyskasz portfela.

