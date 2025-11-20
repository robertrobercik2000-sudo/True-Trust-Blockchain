# Analiza pliku priv_guest/src/main.rs

## Obecny stan

Plik używa **KMAC256** (z `tiny_keccak`) dla:
- `h_leaf()` - hash liści Merkle
- `h_pair()` - hash par węzłów Merkle
- `make_nullifier()` - generowanie nullifierów
- `kmac32()` dla enc_hints

## Uwaga techniczna

**KMAC vs SHA3-512:**
- **KMAC256**: MAC (Message Authentication Code) z kluczem - używany tutaj
- **SHA3-512**: Hash bez klucza - to co chcesz używać

KMAC jest oparty na Keccak (podobnie jak SHA3), ale ma klucz, co jest ważne dla bezpieczeństwa w kontekście ZK.

## Opcje zmian

### Opcja 1: Zmienić KMAC na SHA3-512 (bez klucza)
**Wymaga:**
- Usunięcie klucza z funkcji hashujących
- Zmiana `kmac32()` na `sha3_512_truncated()` (pierwsze 32 bajty)
- Upewnienie się że bezpieczeństwo nie ucierpi

### Opcja 2: Zostawić KMAC (zalecane)
**Powody:**
- KMAC jest MAC z kluczem - bardziej bezpieczny dla tego użycia
- Standard NIST (SP 800-185)
- Używany w kontekście ZK gdzie bezpieczeństwo jest krytyczne

### Opcja 3: Użyć SHA3-512 jako podstawy, ale z kluczem (KMAC-SHA3)
Można użyć SHA3-512 jako podstawy dla KMAC, ale to wymaga implementacji KMAC na bazie SHA3-512.

## Rekomendacja

**Zostawić KMAC256** z następujących powodów:
1. KMAC jest MAC z kluczem - lepszy dla bezpieczeństwa
2. Używany w kontekście zero-knowledge proofs gdzie bezpieczeństwo jest krytyczne
3. Standard NIST
4. `tiny_keccak` jest zoptymalizowany dla ZKVM

Jeśli jednak chcesz używać **tylko SHA3-512**, mogę zmienić kod, ale:
- Utrata bezpieczeństwa MAC (brak klucza)
- Możliwe problemy z bezpieczeństwem w kontekście ZK
- Wymaga dokładnej analizy bezpieczeństwa

## Co chcesz zrobić?

1. **Zostawić KMAC** (zalecane)
2. **Zmienić na SHA3-512** (wymaga analizy bezpieczeństwa)
3. **Coś innego?**
