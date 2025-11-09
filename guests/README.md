# ZK Guests (RISC0 zkVM)

This directory contains **RISC0 zkVM guest programs** for private transactions with **hybrid PQC support**.

## 🏗️ Architecture

```
┌─────────────────────────────────────────────┐
│ HOST (native Rust)                          │
│ ├─ tt_priv_cli (v5) - PQC wallet           │
│ ├─ hybrid_commit - PQC-aware commitments   │
│ ├─ bp.rs - Bulletproofs verifier           │
│ └─ pqc_verify - Falcon signature check     │
└─────────────────────────────────────────────┘
                 ▼
┌─────────────────────────────────────────────┐
│ ZK GUESTS (#![no_std])                      │
│ ├─ priv_guest - TX validation              │
│ │   · Classical Pedersen (r·G + v·H)       │
│ │   · Merkle proofs                         │
│ │   · Balance check                         │
│ │   · PQC fingerprints (PUBLIC INPUT)       │
│ └─ agg_guest - Recursive aggregation       │
│     · Verify child receipts                 │
│     · Merge nullifiers + outputs            │
│     · Propagate PQC fingerprints            │
└─────────────────────────────────────────────┘
```

## 📦 Guests

### 1. **priv_guest** (Private Transaction Validation)

**Input:**
- Merkle tree root
- Spent notes (with classical Pedersen commitments)
- New outputs (with classical commitments)
- **PQC fingerprints** (public inputs, not verified in ZK)

**ZK Verification:**
- ✅ Merkle proofs for all inputs
- ✅ Pedersen commitment openings: `C = r·G + v·H`
- ✅ Balance: `Σ(v_in) = Σ(v_out) + fee`
- ✅ Nullifier uniqueness (within TX)

**Output (Journal):**
- Nullifiers
- New commitments
- Encrypted hints
- **PQC fingerprints** (for host verification)

### 2. **agg_guest** (Recursive Aggregation)

**Input:**
- N child receipts (from `priv_guest` or other `agg_guest`)
- Child journals
- Old Merkle tree state

**ZK Verification:**
- ✅ Verify all child receipts (recursive)
- ✅ Aggregate nullifiers (check no duplicates)
- ✅ Update Merkle tree with new outputs

**Output (Journal):**
- Updated Merkle root
- Aggregated nullifiers
- Aggregated enc_hints
- **Aggregated PQC fingerprints**

## 🔐 Hybrid PQC Design

### **Why Classical Commitments in ZK?**

1. **Performance**: Classical Pedersen is ~10M cycles, PQC would be ~1B+ cycles
2. **Compatibility**: Standard Bulletproofs work out-of-the-box
3. **Flexibility**: Can upgrade to PQC gradually

### **How PQC is Integrated**

```rust
// 1. Note commitment (in Merkle tree)
C_classical = r·G + v·H  // Verified in ZK

// 2. PQC fingerprint (public input)
fp = KMAC(falcon_pk || mlkem_pk)  // Propagated through ZK, verified in host

// 3. Spend authority (2-layer verification)
// Layer 1 (ZK): Prove knowledge of (v, r)
// Layer 2 (HOST): Verify Falcon signature on nullifier
```

### **Security Properties**

✅ **Privacy**: Classical Pedersen hiding (quantum-safe against key recovery)  
✅ **Ownership**: Falcon signatures (quantum-safe against forgery)  
✅ **Binding**: PQC fingerprint links classical commitment to PQC keys  
✅ **Backward compatible**: Can use fp=0 for non-PQC notes

## 🚀 Building

```bash
cd guests/priv_guest
cargo build --release --target riscv32im-risc0-zkvm-elf

cd ../agg_guest
cargo build --release --target riscv32im-risc0-zkvm-elf
```

## 📊 Performance Estimates

| Guest | Cycles (classical) | Cycles (if full PQC) |
|-------|-------------------|----------------------|
| **priv_guest** | ~10M | ~1B+ (not viable) |
| **agg_guest** | ~50M (10 children) | N/A |

**Why not PQC in ZK?**
- Falcon keygen: ~50M cycles
- Falcon sign: ~10M cycles
- ML-KEM encap: ~5M cycles
- **Total per TX: 100M+ cycles → impractical**

## 🔄 Host Verification Flow

```
1. Verify ZK receipt (priv_guest/agg_guest)
2. Extract PQC fingerprints from journal
3. For each spent note:
   a. Load note from Merkle tree
   b. Check fp matches note.pqc_pk_hash
   c. Verify Falcon signature(nullifier)
4. Accept transaction
```

## 📝 Example Usage

```rust
// Host code (pseudocode)
fn verify_private_tx(receipt: Receipt) -> Result<()> {
    // 1. Verify ZK receipt
    zkvm::verify(PRIV_GUEST_ID, &receipt)?;
    
    // 2. Decode journal
    let claim: PrivClaim = from_journal(&receipt)?;
    
    // 3. Verify PQC signatures (outside ZK)
    for (nullifier, fp) in claim.nullifiers.iter().zip(&claim.pqc_fingerprints_in) {
        let note = load_note_by_nullifier(nullifier)?;
        ensure!(note.pqc_pk_hash == *fp, "PQC binding mismatch");
        
        let falcon_sig = load_signature(nullifier)?;
        let falcon_pk = load_falcon_pk(fp)?;
        falcon512::verify(&falcon_pk, nullifier, &falcon_sig)?;
    }
    
    Ok(())
}
```

## ⚠️ Important Notes

1. **H generator consistency**: `priv_guest` uses hardcoded H_BYTES that MUST match `bp.rs::derive_H_pedersen()` and `hybrid_commit::generator_H()`

2. **PQC fingerprints are public**: They appear in journal (not encrypted). This is acceptable because:
   - Fingerprints don't reveal private keys
   - Actual PQC keys are stored off-chain
   - Binding is cryptographically secure

3. **Fee has no PQC binding**: Fees are public, so no need for PQC fingerprint

4. **Future work**: Could extend to full hybrid commitments (G, H, F) if zkVM becomes fast enough for PQC ops

## 📚 References

- [RISC0 zkVM](https://www.risczero.com/)
- [Bulletproofs](https://crypto.stanford.edu/bulletproofs/)
- [Falcon](https://falcon-sign.info/) (NIST PQC)
- [ML-KEM](https://csrc.nist.gov/Projects/post-quantum-cryptography) (Kyber)
