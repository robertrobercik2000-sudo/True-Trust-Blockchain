# 🎨 TRUE TRUST - WIZUALNY PRZEWODNIK

*Obrazki i diagramy wyjaśniające jak działa system*

---

## 🏗️ ARCHITEKTURA SYSTEMU (Widok z góry)

```
┌─────────────────────────────────────────────────────────────────────┐
│                         UŻYTKOWNIK                                  │
│                              │                                       │
│                    ┌─────────┴──────────┐                           │
│                    │                    │                           │
│              ┌─────▼──────┐      ┌─────▼──────┐                    │
│              │  WALLET    │      │   NODE     │                    │
│              │  (Portfel) │      │  (Węzeł)   │                    │
│              └─────┬──────┘      └─────┬──────┘                    │
│                    │                    │                           │
│              [Klucze prywatne]   [Mining + P2P]                     │
└─────────────────────────────────────────────────────────────────────┘
                                   │
                         ┌─────────┴──────────┐
                         │                    │
                    ┌────▼─────┐        ┌────▼─────┐
                    │  Node 1  │◄──────►│  Node 2  │
                    └────┬─────┘        └────┬─────┘
                         │                    │
                    ┌────▼─────┐        ┌────▼─────┐
                    │  Node 3  │◄──────►│  Node 4  │
                    └──────────┘        └──────────┘
                         │
                    ┌────▼──────────────────────────────┐
                    │    BLOCKCHAIN (Łańcuch bloków)    │
                    │  [B1][B2][B3]...[B1000][B1001]    │
                    └───────────────────────────────────┘
```

---

## ⛏️ PROCES KOPANIA (Mining) - TIMELINE

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
                         5-SEKUNDOWY SLOT
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

0ms          ╔═══════════════════════════════════════════╗
             ║  ⏰ NOWY SLOT ZACZYNA SIĘ                 ║
             ╚═══════════════════════════════════════════╝
                              │
0.1ms                    🎲 LOSUJESZ
                              │
                      ┌───────┴────────┐
                      │                │
                  PRZEGRAŁEŚ       WYGRAŁEŚ! ✅
                      │                │
                  ❌ CZEKASZ            │
                   5 sekund        ┌───▼────┐
                      │            │ 1ms    │ Zbierz transakcje
                      │            └───┬────┘
                      │                │
                      │            ┌───▼────┐
                      │            │ 11ms   │ Bulletproofs (cached)
                      │            └───┬────┘
                      │                │
                      │            ┌───▼────┐
                      │            │ 461ms  │ PoZS proof (optional)
                      │            └───┬────┘
                      │                │
                      │            ┌───▼────┐
                      │            │ 471ms  │ Stwórz + podpisz blok
                      │            └───┬────┘
                      │                │
5000ms              ┌─▼────────────────▼─┐
                    │ 📡 BROADCAST BLOCK  │
                    └──────────┬──────────┘
                               │
                    ╔══════════▼═══════════╗
                    ║  💰 DOSTAŁEŚ NAGRODĘ ║
                    ║     50 TT + fees     ║
                    ╚══════════════════════╝

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

---

## 🎲 JAK DZIAŁA LOSOWANIE? (Probabilistic Sortition)

```
KROK 1: KAŻDY NODE RZUCA KOŚCIĄ
═══════════════════════════════════════════════════════════════

    Node A           Node B           Node C
    (10 monet)       (50 monet)       (5 monet)
    Trust: 0.9       Trust: 0.6       Trust: 1.0
         │                │                │
         ▼                ▼                ▼
    Waga: 9.0        Waga: 30.0       Waga: 5.0
         │                │                │
         └────────────────┴────────────────┘
                          │
                   ╔══════▼═══════╗
                   ║ Σwag = 44.0  ║
                   ╚══════╤═══════╝
                          │
         ┌────────────────┼────────────────┐
         ▼                ▼                ▼
    Szansa: 20%      Szansa: 68%     Szansa: 11%


KROK 2: RANDOMIZACJA (RANDAO Beacon)
═══════════════════════════════════════════════════════════════

    ┌──────────────────────────────────────────────┐
    │  Epoka 5, Slot 100                           │
    │                                              │
    │  Beacon seed:                                │
    │  0x3a7f6b2e4c9d1f8e...                      │
    │                                              │
    │  Hash(beacon + slot + node_id):             │
    │    Node A: 0.00034  ← Wylosował nisko! ✅    │
    │    Node B: 0.89423  ← Wylosował wysoko       │
    │    Node C: 0.45612  ← W środku               │
    └──────────────────────────────────────────────┘


KROK 3: SPRAWDZENIE PROGU
═══════════════════════════════════════════════════════════════

    Node A:
    ┌────────────────────────────────────┐
    │  Wylosowana liczba: 0.00034        │
    │  Próg (threshold):  0.00200        │
    │                                    │
    │  0.00034 < 0.00200? TAK! ✅        │
    │                                    │
    │  🎉 NODE A WYGRYWA!                │
    └────────────────────────────────────┘
    
    Node B:
    ┌────────────────────────────────────┐
    │  Wylosowana liczba: 0.89423        │
    │  Próg (threshold):  0.68000        │
    │                                    │
    │  0.89423 < 0.68000? NIE ❌         │
    │                                    │
    │  Node B nie wygrywa tym razem      │
    └────────────────────────────────────┘
```

---

## 🔐 UKRYTE TRANSAKCJE - JAK TO DZIAŁA?

### NORMALNA TRANSAKCJA (Bitcoin):

```
┌────────────────────────────────────────────────────────────┐
│  BLOK #100 - BITCOIN                                       │
├────────────────────────────────────────────────────────────┤
│                                                            │
│  Transakcja 1:                                             │
│    Od:    Ania (1A1zP1...)                                 │
│    Do:    Bartek (1B2yQ2...)                               │
│    Kwota: 5.0 BTC                                          │
│                                                            │
│  👁️ KAŻDY WIDZI:                                           │
│    - Kto wysłał                                            │
│    - Kto dostał                                            │
│    - Ile                                                   │
└────────────────────────────────────────────────────────────┘

         🔍
    ┌─────────┐
    │ SZPIEG  │ ← "Widzę wszystko!"
    └─────────┘
```

---

### UKRYTA TRANSAKCJA (TRUE TRUST):

```
┌────────────────────────────────────────────────────────────┐
│  BLOK #100 - TRUE TRUST                                    │
├────────────────────────────────────────────────────────────┤
│                                                            │
│  Transakcja 1:                                             │
│    Od:    🔒 [UKRYTE]                                      │
│    Do:    🔒 [UKRYTE]                                      │
│    Kwota: 🔒 [UKRYTA]                                      │
│                                                            │
│    Dowód Bulletproof: ✅ "Transakcja jest poprawna"        │
│    Metka Bloom: 0x3A7F                                     │
│                                                            │
│  👁️ SZPIEG WIDZI:                                          │
│    - ??? (nic!)                                            │
└────────────────────────────────────────────────────────────┘

         🔍
    ┌─────────┐
    │ SZPIEG  │ ← "Nie widzę nic! 😢"
    └─────────┘


         👤 ANIA                              👤 BARTEK
    ┌─────────────┐                      ┌─────────────┐
    │ Klucz prywat│                      │ Klucz prywat│
    │ Odszyfrował:│                      │ Odszyfrował:│
    │ "Wysłałam   │                      │ "Dostałem   │
    │  5 monet"   │  ── 5 monet ──►      │  5 monet"   │
    └─────────────┘                      └─────────────┘
    
    "JA WIDZĘ!"                          "JA WIDZĘ!"
```

---

## 🔍 KEYSEARCH - ZNAJDOWANIE SWOICH TRANSAKCJI

### PROCES SKANOWANIA:

```
╔═══════════════════════════════════════════════════════════════╗
║              BLOCKCHAIN (1000 transakcji)                     ║
╚═══════════════════════════════════════════════════════════════╝

[TX1] [TX2] [TX3] ... [TX847] ... [TX998] [TX999] [TX1000]
  │     │     │          │          │       │       │
  │     │     │          │          │       │       │
  
TY SPRAWDZASZ KAŻDĄ:
══════════════════════════════════════════════════════════════

Step 1: TX1
┌────────────────────────────┐
│ Próbuję odszyfrować...     │
│ Klucz prywatny → Hash      │
│ Czy pasuje? ❌ NIE         │
└────────────────────────────┘
   ⏱️ 10ms


Step 2: TX2
┌────────────────────────────┐
│ Próbuję odszyfrować...     │
│ Klucz prywatny → Hash      │
│ Czy pasuje? ❌ NIE         │
└────────────────────────────┘
   ⏱️ 10ms
   
...

Step 847: TX847
┌────────────────────────────┐
│ Próbuję odszyfrować...     │
│ Klucz prywatny → Hash      │
│ Czy pasuje? ✅ TAK!        │
│                            │
│ TWOJA TRANSAKCJA! 🎉       │
│ Dostałeś: 5 monet          │
└────────────────────────────┘
   ⏱️ 10ms
   
RAZEM: 847 × 10ms = 8.47 sekund
```

---

## 🌸 FILTR BLOOM - SZYBKIE WYSZUKIWANIE

### JAK DZIAŁA?

```
KROK 1: TWORZYSZ FILTR
════════════════════════════════════════════════════════════

Twój klucz → Hash1 → Pozycja 3 w filtrze
           → Hash2 → Pozycja 17 w filtrze
           → Hash3 → Pozycja 42 w filtrze

Filtr Bloom (64 bity):
┌─┬─┬─┬─┬─┬─┬─┬─┬─┬─┬─┬─┬─┬─┬─┬─┬─┬─┬─┬─┬─┬─┬─┬─┬─┐
│0│0│0│1│0│0│0│0│0│0│0│0│0│0│0│0│0│1│0│0│0│0│0│0│0│...
└─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┘
    ↑                             ↑                ↑
    3                             17               42


KROK 2: SPRAWDZASZ TRANSAKCJE (SUPER SZYBKO!)
════════════════════════════════════════════════════════════

TX1: Metka 0x1234
     → Hash → Pozycje: [5, 12, 30]
     ┌────────────────────────────┐
     │ Filtr[5]  = 0  ❌          │
     │ Filtr[12] = ?  (nie sprawdz)│
     │ Filtr[30] = ?  (nie sprawdz)│
     │                            │
     │ WYNIK: NA PEWNO nie Twoja  │
     └────────────────────────────┘
     ⏱️ 0.01ms (100x szybciej!)


TX847: Metka 0x3A7F
     → Hash → Pozycje: [3, 17, 42]
     ┌────────────────────────────┐
     │ Filtr[3]  = 1  ✅          │
     │ Filtr[17] = 1  ✅          │
     │ Filtr[42] = 1  ✅          │
     │                            │
     │ WYNIK: MOŻE być Twoja!     │
     │        Sprawdź dokładnie → │
     └────────────────────────────┘
     ⏱️ 0.01ms + 10ms = 10.01ms


PORÓWNANIE:
════════════════════════════════════════════════════════════

Bez filtra Bloom:
  1000 transakcji × 10ms = 10,000ms (10 sekund) ❌

Z filtrem Bloom:
  - 990 odrzuconych: 990 × 0.01ms = 9.9ms
  - 10 sprawdzonych:  10 × 10ms   = 100ms
  - RAZEM: 109.9ms (0.1 sekundy!) ✅
  
  SPEEDUP: 100x szybciej!
```

---

## 🎭 STEALTH ADDRESSES - WIZUALIZACJA

### JAK NORMALNY ADRES DZIAŁA (zła prywatność):

```
               ANIA (normalny Bitcoin)
              Adres: 1A1zP1...
                      │
      ┌───────────────┼───────────────┐
      │               │               │
 [TX1] 2 BTC     [TX2] 3 BTC     [TX3] 5 BTC
      │               │               │
      └───────────────┴───────────────┘
                      │
            🔍 OBSERWATOR widzi:
            "Te 3 transakcje idą do
             tej samej osoby (Ani)!"
```

---

### JAK STEALTH ADDRESS DZIAŁA (świetna prywatność):

```
           ANIA (TRUE TRUST Stealth)
          Klucz główny: [SEKRET]
                 │
    ┌────────────┼────────────┐
    │            │            │
    ▼            ▼            ▼
[TX1] 2 TT   [TX2] 3 TT   [TX3] 5 TT
    │            │            │
    ▼            ▼            ▼
Adres:       Adres:       Adres:
0x1234...    0x5678...    0xABCD...
(jednoraz.)  (jednoraz.)  (jednoraz.)


      🔍 OBSERWATOR widzi:
      "3 różne osoby dostały pieniądze.
       Nie wiem czy to ta sama osoba!"
```

---

### SZCZEGÓŁOWY PROCES:

```
WYSYŁANIE TRANSAKCJI:
═══════════════════════════════════════════════════════════

1. BARTEK chce wysłać 5 monet do ANI

2. BARTEK ma publiczny klucz Ani: P_ania

3. BARTEK generuje losową liczbę: r (SEKRET!)

4. BARTEK oblicza:
   Jednorazowy_adres = P_ania + Hash(r)
   
5. BARTEK wysyła 5 monet na Jednorazowy_adres

6. BARTEK umieszcza w blockchainie:
   Wskazówka = r × G (punkt na krzywej eliptycznej)
   Metka = Hash16(r × P_ania)  ← Do filtra Bloom


ODBIERANIE TRANSAKCJI:
═══════════════════════════════════════════════════════════

7. ANIA skanuje blockchain

8. ANIA widzi Wskazówkę i Metkę

9. ANIA oblicza:
   Sprawdź = Metka == Hash16(s_ania × Wskazówka)?
   gdzie s_ania = klucz prywatny Ani
   
10. Jeśli TAK:
    ANIA oblicza klucz prywatny dla Jednorazowy_adres
    ANIA może wydać te 5 monet! ✅


WIZUALIZACJA:
═══════════════════════════════════════════════════════════

    BARTEK                  BLOCKCHAIN               ANIA
       │                                              │
   ┌───▼────┐                                    ┌───▼────┐
   │ r=rand │                                    │ s_ania │
   └───┬────┘                                    └───┬────┘
       │                                              │
       │ r × G → Wskazówka                           │
       │ Hash(r × P_ania) → Metka                    │
       │                                              │
       ├─────────► [BLOCKCHAIN] ◄────────────────────┤
       │            [Wskazówka]                       │
       │            [Metka]                           │
       │            [5 monet]                         │
       │                                              │
       │                                    s_ania × Wskazówka
       │                                    == r × G × s_ania
       │                                    == Hash(r × P_ania)
       │                                    == Metka ✅
       │                                              │
       │                                    Odszyfrował!
       │                                    Dostał 5 monet!
```

---

## 🔒 BULLETPROOF - WIZUALIZACJA

### CO TO UDOWADNIA?

```
╔═══════════════════════════════════════════════════════════╗
║  BULLETPROOF DOWODZI:                                     ║
║  1. Kwota jest ≥ 0 (nie ujemna)                           ║
║  2. Kwota jest ≤ 2^64 (realistyczna)                      ║
║  3. Nadawca MA tę kwotę                                   ║
║  4. Bilans się zgadza (in = out)                          ║
║                                                           ║
║  BEZ UJAWNIANIA:                                          ║
║  - Dokładnej kwoty                                        ║
║  - Salda nadawcy                                          ║
║  - Salda odbiorcy                                         ║
╚═══════════════════════════════════════════════════════════╝
```

---

### JAK DZIAŁA? (Uproszczona analogia)

```
PEDERSEN COMMITMENT (Zobowiązanie)
═══════════════════════════════════════════════════════════

Ania ma: 50 monet (SEKRET!)
Ania chce udowodnić: "Mam 0-100 monet"

1. Ania wybiera losową liczbę: r = 42 (SEKRET!)

2. Ania oblicza commitment:
   C = r × G + 50 × H
   gdzie G i H to punkty na krzywej eliptycznej
   
3. Ania publikuje: C

4. KAŻDY widzi C, ale NIE WIE ile jest monet!
   (bo r jest sekretne)


BULLETPROOF (Dowód zakresu)
═══════════════════════════════════════════════════════════

5. Ania generuje Bulletproof:
   Dowód = [matematyczny dowód że 50 ∈ [0, 100]]
   Rozmiar: 672 bajty
   
6. Ania publikuje: (C, Dowód)

7. WERYFIKATOR sprawdza:
   ✅ Czy Dowód pasuje do C?
   ✅ Czy C reprezentuje wartość 0-100?
   
   Jeśli TAK → Transakcja OK!
   Jeśli NIE → Transakcja odrzucona!


WIZUALIZACJA:
═══════════════════════════════════════════════════════════

┌─────────────────────────────────────────────────────────┐
│  ANIA                                                   │
│  ┌──────────────┐                                       │
│  │ 50 monet     │ ← SEKRET (nie publikuje)              │
│  │ r = 42       │ ← SEKRET (nie publikuje)              │
│  └──────┬───────┘                                       │
│         │                                               │
│         ▼                                               │
│  C = r×G + 50×H  ← PUBLIKUJE                            │
│  Bulletproof     ← PUBLIKUJE (672 bajty)                │
└─────────┬───────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────┐
│  BLOCKCHAIN                                             │
│  [C: 0x3a7f...]                                         │
│  [Proof: 0xabcd... (672 bytes)]                         │
└─────────┬───────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────┐
│  WERYFIKATOR                                            │
│  ┌──────────────────────────────┐                       │
│  │ Sprawdź dowód...             │                       │
│  │ Czy C ∈ [0, 2^64]? ✅        │                       │
│  │ Czy dowód poprawny? ✅       │                       │
│  └──────────────────────────────┘                       │
│                                                         │
│  WYNIK: Transakcja OK!                                  │
│         (Ale NIE WIE że to 50!)                         │
└─────────────────────────────────────────────────────────┘
```

---

## 🎖️ PROOF-OF-TRUST - DYNAMIKA ZAUFANIA

### JAK ZMIENIA SIĘ TRUST?

```
╔═══════════════════════════════════════════════════════════╗
║              SYMULACJA PRZEZ 20 BLOKÓW                    ║
╚═══════════════════════════════════════════════════════════╝

Parametry:
  α (alpha) = 0.95 (decay)
  β (beta)  = 0.05 (reward)


NODE A (Uczciwy):
═══════════════════════════════════════════════════════════

Blok  │ Wygrał? │ Trust przed │ Trust po   │ Zmiana
──────┼─────────┼─────────────┼────────────┼────────
  1   │   TAK   │   0.500     │   0.525    │  +5%
  2   │   NIE   │   0.525     │   0.499    │  -5%
  3   │   TAK   │   0.499     │   0.524    │  +5%
  4   │   TAK   │   0.524     │   0.548    │  +5%
  5   │   NIE   │   0.548     │   0.520    │  -5%
  6   │   TAK   │   0.520     │   0.544    │  +5%
  7   │   TAK   │   0.544     │   0.567    │  +5%
  8   │   TAK   │   0.567     │   0.589    │  +4%
  9   │   NIE   │   0.589     │   0.560    │  -5%
 10   │   TAK   │   0.560     │   0.582    │  +4%
 ...
 20   │   TAK   │   0.645     │   0.663    │  +3%

TREND: ↗️ Rośnie (bo często wygrywa i gra uczciwie)


NODE B (Średnio aktywny):
═══════════════════════════════════════════════════════════

Blok  │ Wygrał? │ Trust przed │ Trust po   │ Zmiana
──────┼─────────┼─────────────┼────────────┼────────
  1   │   NIE   │   0.500     │   0.475    │  -5%
  2   │   NIE   │   0.475     │   0.451    │  -5%
  3   │   NIE   │   0.451     │   0.428    │  -5%
  4   │   TAK   │   0.428     │   0.457    │  +7%
  5   │   NIE   │   0.457     │   0.434    │  -5%
  ...
 20   │   NIE   │   0.380     │   0.361    │  -5%

TREND: ↘️ Spada (rzadko wygrywa, trust maleje)


NODE C (Oszust - próbował oszukać w bloku 10):
═══════════════════════════════════════════════════════════

Blok  │ Event   │ Trust przed │ Trust po   │ Zmiana
──────┼─────────┼─────────────┼────────────┼────────
  1   │  Norm   │   0.600     │   0.620    │  +3%
  2   │  Norm   │   0.620     │   0.589    │  -5%
  ...
  9   │  Norm   │   0.650     │   0.668    │  +3%
 10   │ OSZUST! │   0.668     │   0.168    │  -50% ❌
 11   │  Norm   │   0.168     │   0.160    │  -5%
 12   │  Norm   │   0.160     │   0.152    │  -5%
 ...
 20   │  Norm   │   0.110     │   0.105    │  -5%

TREND: ⬇️⬇️ Drastyczny spadek! Oszukiwanie się nie opłaca!
```

---

### WYKRES TRUSTOWANIA:

```
Trust
 1.0  │
      │                    
 0.9  │                              ╱╱─╲
      │                         ╱───╱    ╲
 0.8  │                    ╱───╱           ╲___
      │               ╱───╱                    ╲
 0.7  │          ╱───╱                          ╲──
      │     ╱───╱                                  ╲  NODE A
 0.6  │╱───╱                                        ╲ (uczciwy)
      ├────────────────────────────────────────────────────
 0.5  │        ╲╲                                      
      │          ╲╲___                                 NODE B
 0.4  │               ╲╲___                          (średnio)
      │                    ╲╲___
 0.3  │                         ╲╲___
      │                              ╲╲___
 0.2  │                     ⚡OSZUSTWO!  ╲
      │                         │         ╲╲╲___     NODE C
 0.1  │                         │              ╲╲╲___ (oszust)
      │                         │                   ╲╲╲
 0.0  └─────────────────────────┼──────────────────────────►
      0    5    10   15   20   25   30   35   40   Bloki
                                ▲
                         Blok 10: Node C oszukał
                         i stracił 50% trust!
```

---

## 📱 INTERFEJS UŻYTKOWNIKA (Przykłady ekranów)

### EKRAN GŁÓWNY PORTFELA:

```
┌───────────────────────────────────────────────────────────┐
│  💼 TRUE TRUST WALLET v5.0                         [≡]    │
├───────────────────────────────────────────────────────────┤
│                                                           │
│   💰 TWOJE SALDO                                          │
│   ┌─────────────────────────────────────────────────┐    │
│   │                                                 │    │
│   │          1,234.56 TT                            │    │
│   │          ≈ $12,345.60 USD                       │    │
│   │                                                 │    │
│   └─────────────────────────────────────────────────┘    │
│                                                           │
│   🎖️  ZAUFANIE (TRUST): ████████░░ 85%                   │
│                                                           │
│   ┌─────────────────┬───────────────────┐               │
│   │  📤 WYŚLIJ      │   📥 ODBIERZ      │               │
│   └─────────────────┴───────────────────┘               │
│                                                           │
│   📊 OSTATNIE TRANSAKCJE                                 │
│   ┌─────────────────────────────────────────────────┐    │
│   │ Dziś 14:23   +50.00 TT    Block reward          │    │
│   │ Dziś 12:15   -10.00 TT    Wysłano               │    │
│   │ Wczoraj      +52.50 TT    Block reward          │    │
│   │ Wczoraj      +48.00 TT    Block reward          │    │
│   └─────────────────────────────────────────────────┘    │
│                                                           │
│   [Zobacz więcej ▼]                                      │
│                                                           │
└───────────────────────────────────────────────────────────┘
```

---

### EKRAN STATUS NODE'A:

```
┌───────────────────────────────────────────────────────────┐
│  ⛏️  NODE STATUS - moj_node                        [≡]    │
├───────────────────────────────────────────────────────────┤
│                                                           │
│   🟢 ONLINE - Running                                     │
│                                                           │
│   📊 BLOCKCHAIN                                           │
│   ┌─────────────────────────────────────────────────┐    │
│   │  Wysokość (Height): 12,345                      │    │
│   │  Twoje bloki:       142                         │    │
│   │  Sync status:       100% ████████████           │    │
│   │  Ostatni blok:      12 sekund temu              │    │
│   └─────────────────────────────────────────────────┘    │
│                                                           │
│   ⛏️  MINING                                              │
│   ┌─────────────────────────────────────────────────┐    │
│   │  Twoje zaufanie:    0.85  ████████░░            │    │
│   │  Stake:             1,000 TT                    │    │
│   │  Szansa na blok:    ~8% (co 125 sekund)        │    │
│   │  Ostatni blok:      Slot 1234 (5 min temu)     │    │
│   └─────────────────────────────────────────────────┘    │
│                                                           │
│   🌐 NETWORK                                              │
│   ┌─────────────────────────────────────────────────┐    │
│   │  Połączeni peers:   12                          │    │
│   │  Mempool:           24 txs                      │    │
│   │  Uptime:            3d 14h 23m                  │    │
│   └─────────────────────────────────────────────────┘    │
│                                                           │
│   [Szczegóły ▼]  [Restart]  [Stop]                      │
│                                                           │
└───────────────────────────────────────────────────────────┘
```

---

*Wizualny przewodnik stworzony dla TRUE TRUST Blockchain v5.0.0*  
*Diagramy i ASCII art dla lepszego zrozumienia*  
*Pytania? Zobacz USER_GUIDE_PL.md lub GitHub Issues!*
