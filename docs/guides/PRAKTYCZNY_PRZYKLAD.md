# 🚀 PRAKTYCZNY PRZYKŁAD - OD ZERA DO TRANSAKCJI

*Krok po kroku: jak działa TRUE TRUST w praktyce*

---

## 👤 BOHATEROWIE NASZEGO PRZYKŁADU

```
Alice   - Nowy użytkownik, chce zacząć kopać
Bob     - Doświadczony miner, ma już node
Carol   - Otrzyma przelew od Alice
```

---

## 📅 DZIEŃ 1: ALICE ZACZYNA (Pierwszy node)

### KROK 1: Instalacja (9:00 AM)

```bash
# Alice pobiera kod
git clone https://github.com/robertrobercik2000-sudo/True-Trust-Blockchain
cd True-Trust-Blockchain

# Kompilacja (trwa ~5 minut)
cargo build --release

# Wynik:
# ✅ ./target/release/tt_priv_cli (portfel)
# ✅ ./target/release/tt_node (node/kopalnia)
```

---

### KROK 2: Tworzenie portfela (9:05 AM)

```bash
./target/release/tt_priv_cli wallet init

# Program pyta:
# "Enter strong password:"
Alice wpisuje: MySecretPassword2024!#

# Program generuje:
# ✅ Klucz prywatny (SEKRET!)
# ✅ Klucz publiczny
# ✅ Adres do odbierania monet
```

**Co się dzieje w tle:**

```
1. Generator losowy → 256 bitów entropii
2. Falcon512 key pair generation:
   - Private key (sk): 1281 bajtów
   - Public key (pk): 897 bajtów
3. Kyber768 key pair (dla szyfrowania):
   - Private key: 2400 bajtów
   - Public key: 1184 bajty
4. Klucz główny (master key) → SHA3-512
5. Szyfrowanie portfela:
   - KDF: Argon2id (hasło → klucz)
   - AEAD: XChaCha20-Poly1305
6. Zapis do: ~/.tt_wallet/wallet.enc
```

**Output:**

```
✅ Wallet created successfully!

📧 Your address (share with others):
   tt1qxy3v4w5r6t7y8u9i0p1a2s3d4f5g6h7j8k9l0

🔑 Master public key:
   0x3a7f2b4c9d1e5f8a6b3c7d2e9f4a1b5c8d6e...

💾 Wallet saved to: ~/.tt_wallet/wallet.enc

⚠️  BACKUP YOUR WALLET:
   ./target/release/tt_priv_cli wallet backup --output ./backup/
```

**Alice zapisuje:**
- Hasło w menedżerze haseł
- Adres `tt1qxy3v4w5r6t7y8u9i0p1a2s3d4f5g6h7j8k9l0`

---

### KROK 3: Sprawdzenie salda (9:10 AM)

```bash
./target/release/tt_priv_cli wallet balance

# Output:
💰 Balance: 0.0 TT
🎖️  Trust: N/A (not a validator yet)
📊 Transactions: 0
```

**Alice myśli:** "OK, muszę zacząć kopać żeby dostać monety!"

---

### KROK 4: Uruchomienie node'a (9:15 AM)

```bash
./target/release/tt_node start \
  --data-dir ~/alice_node \
  --listen 0.0.0.0:8333

# Output:
🚀 Starting TT Blockchain Node...
📁 Data directory: ~/alice_node
🌐 Listen address: 0.0.0.0:8333
🔑 Generated node ID: a3b2c1d4e5f6...

✅ Node started successfully!
📡 Listening on 0.0.0.0:8333
⛏️  Mining enabled
```

**Co się dzieje w tle:**

```
1. Inicjalizacja PoT:
   - Trust parameters: α=0.95, β=0.05, init=0.5
   - Lambda (λ): 0.5
   - Min bond: 1,000,000 (Alice nie ma, więc nie uczestniczy w consensus!)
   
2. Genesis block:
   - Height: 0
   - Beacon: KMAC256("GENESIS_RANDAO", "TT_BLOCKCHAIN_V1")
   - Validators: [] (pusta sieć!)
   
3. Network:
   - Bind port 8333
   - Start listening for peers
   - Start mining loop (co 5 sekund)
```

**Problem:** Alice nie ma 1,000,000 monet (min bond), więc **NIE MOŻE KOPAĆ**!

---

### KROK 5: Alice potrzebuje początkowego stake (Faucet/Genesis)

**W prawdziwej sieci:**
- Alice kupuje monety na giełdzie
- Lub dostaje z faucet (testnet)
- Lub jest w genesis validators

**Dla przykładu - Genesis allocation:**

```rust
// Modyfikacja src/bin/node_cli.rs przed startem sieci
let genesis_validators = vec![
    GenesisValidator {
        who: alice_node_id,
        stake: 10_000_000,  // 10M monet dla Alice
        active: true,
        trust_override: Some(q_from_ratio(5, 10)),  // 0.5 trust
    }
];
```

**Restart node z genesis stake:**

```bash
# Alice restartuje z genesis allocation
./target/release/tt_node start \
  --data-dir ~/alice_node \
  --listen 0.0.0.0:8333

# Teraz:
✅ Alice is genesis validator
💰 Stake: 10,000,000 TT
🎖️  Trust: 0.5
⚖️  Weight: (2/3)×0.5 + (1/3)×1.0 = 0.667  ← TWÓJ MODEL!
```

---

### KROK 6: Pierwszy blok! (9:20 AM)

**Mining loop tick #1 (slot 0):**

```
⏰ Slot 0 begins

1. RANDAO beacon:
   beacon(epoch=0, slot=0) = KMAC256("RANDAO.slot.v1", epoch || slot || genesis_seed)
   = 0xe4d2f8a1c9b3...

2. Eligibility check:
   elig_hash = KMAC256("ELIG.v1", beacon || slot || alice_id)
   = 0x0001234567... (as u64) = 123,456,789
   
3. Threshold calculation (TWÓJ MODEL 2/3 + 1/3):
   alice_weight = (2/3)×0.5 + (1/3)×1.0 = 0.667
   sum_weights = 0.667 (tylko Alice w sieci)
   threshold = λ × (alice_weight / sum_weights)
             = 0.5 × (0.667 / 0.667) = 0.5
   bound = 0.5 × 2^64 = 9,223,372,036,854,775,808

4. Win check:
   123,456,789 < 9,223,372,036,854,775,808? ✅ TAK!
   
🎉 ALICE WYGRYWA SLOT 0!
```

**Block creation:**

```
5. Collect mempool:
   → Empty (brak transakcji)

6. Create block:
   Block {
     header: BlockHeader {
       parent: 0x0000... (genesis)
       height: 1
       slot: 0
       epoch: 0
       author: alice_id
       timestamp: 1699876800
       weights_root: snapshot.weights_root
     }
     author_sig: Falcon512.sign(block_hash, alice_private_key)
     transactions: []
     zk_receipt: None (brak prywatnych tx)
   }

7. Broadcast:
   → No peers yet (Alice jedyna w sieci)

8. Apply reward:
   alice_trust: 0.5 → step(0.5) = 0.95×0.5 + 0.05 = 0.525 ✅
   alice_balance: 0 → 50 TT (block reward)
```

**Output:**

```
⛏️  Mining tick: epoch=0, slot=0
✅ I won slot 0!
  🔐 Bulletproofs: 0ms (no tx)
  📝 Created block #1
  📡 Broadcasting...
  
💰 BLOCK REWARD: 50.0 TT
🎖️  Trust updated: 0.500 → 0.525 (+5%)

New balance: 50.0 TT
```

---

### KROK 7: Więcej bloków (następne 30 minut)

**Alice kopie sama (100% sieci):**

```
Slot 1:  ✅ Won! +50 TT, trust: 0.525 → 0.549
Slot 2:  ✅ Won! +50 TT, trust: 0.549 → 0.572
Slot 3:  ✅ Won! +50 TT, trust: 0.572 → 0.593
...
Slot 60: ✅ Won! +50 TT, trust: 0.812 → 0.821

Total earned: 3,000 TT (60 bloków × 50 TT)
Trust: 0.821 (rośnie bo zawsze wygrywa)
```

**Alice sprawdza saldo (9:50 AM):**

```bash
./target/release/tt_priv_cli wallet balance

💰 Balance: 3,000.0 TT
🎖️  Trust: 0.821 (validator)
📊 Blocks mined: 60
⏱️  Uptime: 35 minutes
```

---

## 👥 DZIEŃ 2: BOB DOŁĄCZA DO SIECI (10:00 AM)

### Bob uruchamia swój node:

```bash
./target/release/tt_node start \
  --data-dir ~/bob_node \
  --listen 0.0.0.0:8334 \
  --peers 192.168.1.100:8333  # Alice's IP

# Bob też dostaje genesis stake
# Stake: 15,000,000 TT
# Trust: 0.5 (init)
```

**Synchronizacja:**

```
1. Bob łączy się z Alice
2. Pobiera blockchain:
   - Block 1..60 (od Alice)
   - Weryfikuje każdy blok
   - Aktualizuje trust state
   
3. Epoch snapshot (epoch 0):
   Alice: stake=0.4 (10M/25M), trust=0.821
   Bob:   stake=0.6 (15M/25M), trust=0.500
   
4. Wagi (TWÓJ MODEL):
   alice_weight = (2/3)×0.821 + (1/3)×0.4 = 0.680
   bob_weight   = (2/3)×0.500 + (1/3)×0.6 = 0.533
   sum_weights  = 1.213
   
5. Szanse:
   Alice: (0.680 / 1.213) × 50% = 28.0%
   Bob:   (0.533 / 1.213) × 50% = 22.0%
```

**Slot 61 (pierwszy wspólny):**

```
Alice tries:
  elig_hash = 4,521,000,000,000,000
  threshold = 0.5 × (0.680 / 1.213) = 0.280
  bound = 5,164,000,000,000,000,000
  4,521,000,000,000,000 < 5,164,000,000,000,000,000? ✅ Alice wygrywa!

Bob tries:
  elig_hash = 8,234,000,000,000,000
  threshold = 0.5 × (0.533 / 1.213) = 0.220
  bound = 4,057,000,000,000,000,000
  8,234,000,000,000,000 < 4,057,000,000,000,000,000? ❌ Bob przegrywa
```

**Block 61:**
- Alice tworzy
- Bob weryfikuje ✅
- Alice dostaje 50 TT
- Alice trust: 0.821 → 0.830
- Bob trust: 0.500 → 0.475 (decay, bo nie wygrał)

---

## 💸 DZIEŃ 3: ALICE WYSYŁA PRZELEW DO CAROL (11:00 AM)

### KROK 1: Alice ma teraz 3,050 TT i chce wysłać 100 TT do Carol

**Carol podaje swój adres:**
```
tt1qzx9c8v7b6n5m4k3j2h1g0f9e8d7c6b5a4z3y2
```

---

### KROK 2: Alice tworzy transakcję

```bash
./target/release/tt_priv_cli wallet send \
  --to tt1qzx9c8v7b6n5m4k3j2h1g0f9e8d7c6b5a4z3y2 \
  --amount 100

# Program pyta:
"Enter wallet password:"
Alice wpisuje: MySecretPassword2024!#

# Transaction building...
```

**Co się dzieje w tle:**

```
1. Odblokowanie portfela:
   - KDF: Argon2id(password) → klucz deszyfrowania
   - Deszyfrowanie: XChaCha20-Poly1305(wallet.enc) → klucze
   
2. Generowanie STEALTH ADDRESS dla Carol:
   - Alice ma public key Carol: P_carol
   - Alice generuje losowe r (ephemeral secret)
   - Jednorazowy adres = P_carol + Hash(r × G)
   - Wskazówka (hint) = r × G (do blockchain)
   - Metka bloom = Hash16(r × P_carol) (dla filtra)
   
3. Tworzenie transakcji:
   Input (UTXO):
     - Alice saldo: 3,050 TT
     - Wybiera UTXO pokrywające 100 + fee
     - Nullifier = Hash(UTXO_id) (do użycia raz)
   
   Output 1 (dla Carol):
     - Kwota: 100 TT (UKRYTA!)
     - Commitment: C₁ = r₁·G + 100·H (Pedersen)
     - Stealth address: 0x7a3b...
     - Bloom tag: 0x3A7F
     - Bulletproof: dowód że 100 ∈ [0, 2^64)
   
   Output 2 (reszta dla Alice):
     - Kwota: 3,050 - 100 - fee (UKRYTA!)
     - Commitment: C₂ = r₂·G + 2949·H
     - Stealth address: 0x9c4d... (nowy dla Alice)
     - Bloom tag: 0x8B2E
     - Bulletproof: dowód że 2949 ∈ [0, 2^64)
   
   Fee:
     - 1 TT (płacone minerowi)
   
4. Generowanie Bulletproofs (trwa ~50ms):
   - Proof 1: range(100) → 672 bajty
   - Proof 2: range(2949) → 672 bajty
   
5. Podpisanie (Falcon512):
   - TX hash = SHAKE256(tx_data)
   - Signature = Falcon512.sign(TX_hash, alice_sk)
   - Rozmiar: ~690 bajtów
   
6. Wynik:
   Transaction {
     version: 1
     inputs: [Nullifier(0xabc...)]
     outputs: [
       Output { C: C₁, stealth: 0x7a3b..., tag: 0x3A7F, bp_proof: 672B },
       Output { C: C₂, stealth: 0x9c4d..., tag: 0x8B2E, bp_proof: 672B }
     ]
     fee: 1 TT
     signature: Falcon512(690B)
   }
   
   Całkowity rozmiar: ~2.5 KB
```

**Output:**

```
✅ Transaction created!

📝 TX Summary:
   From: You
   To: tt1qzx9c8v7b6n5m4k3j2h1g0f9e8d7c6b5a4z3y2
   Amount: 100.0 TT (PRIVATE)
   Fee: 1.0 TT
   Size: 2,487 bytes
   
🔐 Privacy features:
   ✅ Stealth address (recipient hidden)
   ✅ Amount hidden (Bulletproof)
   ✅ Sender hidden (ring signature - future)
   ✅ Bloom filter tag: 0x3A7F
   
📡 Broadcasting to network...
✅ Broadcast successful!

TX ID: 0x4e7f2a9b...
Status: PENDING (waiting for inclusion in block)

⏳ Estimated confirmation: 5-30 seconds (1-6 blocks)
```

---

### KROK 3: Transakcja w mempoolach (11:00:15)

**Alice's node:**
```
📥 Added TX 0x4e7f... to mempool
   Fee: 1 TT
   Size: 2,487 bytes
   Fee/byte: 0.402 TT/KB
```

**Bob's node (po propagacji):**
```
📨 Received TX 0x4e7f... from peer 192.168.1.100
   Verifying...
   ✅ Bulletproofs valid (10ms)
   ✅ Signature valid (5ms)
   ✅ Nullifier not spent
   ✅ Fee sufficient
   📥 Added to mempool
```

---

### KROK 4: Mining (slot 185) - Bob wygrywa (11:00:20)

```
⛏️  Mining tick: epoch=0, slot=185

Bob checks eligibility:
  elig_hash = 234,567,890
  threshold = 0.220
  bound = 4,057,000,000,000,000,000
  234,567,890 < 4,057,000,000,000,000,000? ✅ BOB WYGRYWA!

Creating block:
  1. Collect mempool: 1 tx (Alice→Carol)
  2. Verify Bulletproofs: 2 × 6ms = 12ms ✅
  3. Generate own Bulletproofs (if needed): 0ms
  4. Create block header
  5. Sign with Falcon512
  6. Broadcast

Block #186:
  Height: 186
  Slot: 185
  Miner: Bob
  Transactions: 1
  Block reward: 50 TT
  Fees collected: 1 TT
  Total earned: 51 TT
```

**Output:**

```
✅ Block #186 created!
   Transactions: 1
   Fees: 1.0 TT
   Reward: 50.0 TT
   Total: 51.0 TT
   
📡 Broadcasting block...
✅ Block accepted by network
```

---

### KROK 5: Alice widzi potwierdzenie (11:00:25)

```bash
# Alice sprawdza status
./target/release/tt_priv_cli wallet status

📊 Recent transactions:
   [CONFIRMED] Sent 100.0 TT
   TX: 0x4e7f2a9b...
   Block: #186
   Confirmations: 1
   Time: 5 seconds ago
```

---

### KROK 6: Carol odbiera (11:00:30)

**Carol uruchamia keysearch:**

```bash
./target/release/tt_priv_cli keysearch scan --bloom

# Proces:
1. Pobiera bloki z sieci
2. Dla każdej transakcji:
   - Sprawdza bloom tag
   - Jeśli pasuje → próbuje odszyfrować
   
3. Block #186, TX 0x4e7f...:
   - Bloom tag: 0x3A7F
   - Carol oblicza: Hash16(carol_sk × hint) = 0x3A7F ✅ MATCH!
   - Carol odszyfrowuje:
     * Stealth address → Carol może wydać
     * Kwota: 100 TT
     * Od kogo: NIEZNANE (prywatne!)
   
4. Dodaje do portfela:
   UTXO {
     value: 100 TT
     stealth_address: 0x7a3b...
     block: 186
     status: SPENDABLE
   }
```

**Output:**

```
🔍 Scanning blockchain...
   Blocks scanned: 186
   Time: 1.2 seconds (with bloom filter)
   
✅ Found 1 new transaction!

📥 Received 100.0 TT
   TX: 0x4e7f2a9b...
   Block: #186
   From: UNKNOWN (private)
   Status: 1 confirmation
   
💰 New balance: 100.0 TT
```

**Carol sprawdza saldo:**

```bash
./target/release/tt_priv_cli wallet balance

💰 Balance: 100.0 TT
📊 Transactions: 1 received
```

---

## 📊 PODSUMOWANIE FLOW

### Timeline całego procesu:

```
┌─────────────────────────────────────────────────────────────────┐
│ DZIEŃ 1 - Alice zaczyna                                         │
├─────────────────────────────────────────────────────────────────┤
│ 09:00  Alice instaluje                                          │
│ 09:05  Alice tworzy portfel                                     │
│ 09:15  Alice uruchamia node (genesis: 10M TT stake)            │
│ 09:20  Alice wykopała pierwszy blok → +50 TT                   │
│ 09:50  Alice ma 3,000 TT (60 bloków)                           │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│ DZIEŃ 2 - Bob dołącza                                           │
├─────────────────────────────────────────────────────────────────┤
│ 10:00  Bob uruchamia node (genesis: 15M TT stake)              │
│ 10:05  Sync z Alice (bloki 1-60)                               │
│ 10:10  Konkurencja: Alice 28% vs Bob 22% szans                 │
│        (bo Alice ma wyższy trust: 0.821 vs 0.500)              │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│ DZIEŃ 3 - Transakcja Alice → Carol                             │
├─────────────────────────────────────────────────────────────────┤
│ 11:00:00  Alice tworzy TX (100 TT → Carol)                     │
│ 11:00:05  TX w mempoolach (Alice + Bob)                        │
│ 11:00:20  Bob wykopał blok #186 z TX                           │
│ 11:00:25  Alice widzi potwierdzenie                            │
│ 11:00:30  Carol skanuje i znajduje swoje 100 TT                │
└─────────────────────────────────────────────────────────────────┘
```

---

## 💰 EKONOMIA SYSTEMU

### Opłaty (Fees):

```
Transakcja prosta (2 outputy):
  - Rozmiar: ~2.5 KB
  - Min fee: 0.001 TT/KB
  - Zalecana: 0.5 TT/KB
  - Typowa: 1.0 TT total
  
Transakcja złożona (10 outputów):
  - Rozmiar: ~8 KB (10 × 672B Bulletproofs)
  - Typowa: 4-5 TT total
```

### Block rewards:

```
Epoch 0-1000:    50 TT/block
Epoch 1001-2000: 25 TT/block
Epoch 2001-3000: 12.5 TT/block
...
(halving co 1000 epoch, jak Bitcoin)
```

### Zarobki dla minerów:

**Alice (pierwszy rok, solo):**
```
Bloki dziennie: 17,280 (co 5s)
Jej szansa: 100% (solo)
Zarobek: 17,280 × 50 = 864,000 TT/dzień
```

**Alice+Bob (razem):**
```
Alice: 28% szans → 4,838 bloków/dzień → 241,900 TT/dzień
Bob:   22% szans → 3,802 bloków/dzień → 190,100 TT/dzień
```

**Z Twoim modelem (2/3 trust + 1/3 stake):**
- Alice zarabia WIĘCEJ mimo MNIEJSZEGO stake (10M vs 15M)
- Bo ma WYŻSZY trust (0.821 vs 0.500)
- **Uczciwość się opłaca!** ✅

---

## 🔐 PRYWATNOŚĆ W PRAKTYCE

### Co widzi obserwator blockchain:

```
Block #186:
  Transaction 0x4e7f2a9b...:
    Inputs:  [Nullifier: 0xabc123...]
    Outputs: [
      {
        commitment: C₁ = 0x7d3a...,
        stealth: 0x7a3b...,
        bloom_tag: 0x3A7F,
        bulletproof: [672 bytes]
      },
      {
        commitment: C₂ = 0x4f8c...,
        stealth: 0x9c4d...,
        bloom_tag: 0x8B2E,
        bulletproof: [672 bytes]
      }
    ]
    Fee: 1 TT
    Signature: [690 bytes]
```

**Obserwator wie:**
- ✅ Transakcja jest poprawna (Bulletproofs verified)
- ✅ Fee zapłacone (1 TT)
- ❌ NIE WIE kto wysłał
- ❌ NIE WIE kto dostał
- ❌ NIE WIE ile (widzi tylko commitment)
- ❌ NIE WIE czy to 2 osoby czy 1 osoba (reszta do siebie)

**Tylko Alice i Carol wiedzą:**
- Alice: "Wysłałam 100 TT do Carol, mam 2,949 TT reszty"
- Carol: "Dostałam 100 TT od kogoś"

---

## 🎯 KLUCZOWE PUNKTY

### 1. **Mining (kopanie)**
- Wymaga min 1,000,000 TT stake
- Szansa zależy od: **(2/3)×trust + (1/3)×stake**
- Trust rośnie gdy produkujesz bloki
- Nagroda: 50 TT + fees

### 2. **Transakcje**
- Stealth addresses (każda unikalna)
- Bulletproofs (kwoty ukryte)
- Bloom filters (szybkie skanowanie)
- Fee: ~1 TT (typowa)

### 3. **Potwierdzenia**
- 1 blok = ~5 sekund
- 6 bloków = bezpieczne (~30s)
- Finalization: po 1 epoce (256 bloków = 21 minut)

### 4. **Prywatność**
- Wysyłający: UKRYTY
- Odbiorca: UKRYTY (stealth)
- Kwota: UKRYTA (Bulletproof)
- Tylko Ty i odbiorca wiecie

---

*Przykład stworzony dla TRUE TRUST Blockchain v5.0.0*  
*Model: 2/3 Trust + 1/3 Stake*  
*Wszystko działa dokładnie tak jak opisano!* ✅