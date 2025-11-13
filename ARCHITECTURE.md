# TRUE_TRUST Blockchain - Architecture Documentation

## System Overview

TRUE_TRUST is a post-quantum blockchain combining three innovative layers:

1. **PoT (Proof-of-Trust)** - Consensus layer with adaptive trust scoring
2. **PoZS (Proof-of-ZK-Shares)** - Optional zkSNARK verification layer  
3. **PQ Wallet** - Post-quantum cryptography for long-term security

---

## Layer 1: PoT Consensus

### Core Concepts

**Validator Weight Formula:**
```
weight_q = stake_q × trust_q
```

Where:
- `stake_q`: Validator's locked tokens (u128)
- `trust_q`: Dynamic trust score (Q32.32 fixed-point, range 0.01-2.0)
- Result stored in Merkle tree for each epoch

**Leader Selection Algorithm:**

```rust
fn is_leader(beacon: [u8;32], slot: u64, who: NodeId, weight: Q, total: u128) -> bool {
    // 1. Compute eligibility hash
    let elig = kmac256_hash(b"ELIG.v1", &[beacon, slot.to_le_bytes(), who]);
    let elig_u64 = u64::from_be_bytes(elig[..8]);
    
    // 2. Compute threshold
    let threshold_q = (lambda_q × weight) / total_weight;
    let bound = threshold_q × u64::MAX;
    
    // 3. Check eligibility
    elig_u64 < bound
}
```

**Key Properties:**
- Probabilistic selection (multiple leaders possible per slot)
- Verifiable by anyone with snapshot + beacon
- Resistant to grinding attacks (RANDAO beacon)
- Fair: probability ∝ weight

### RANDAO Beacon

**Commit Phase (Epoch N-1):**
```rust
secret = random_bytes(32);
commitment = kmac256_hash(b"RANDAO.COMMIT", &[secret]);
broadcast(commitment);
```

**Reveal Phase (Epoch N):**
```rust
broadcast(secret);
verify(hash(secret) == commitment);
```

**Finalization:**
```rust
beacon_value = XOR of all revealed secrets
slot_beacon = kmac256_hash(b"BEACON.SLOT", &[beacon_value, epoch, slot])
```

**Security:**
- Last revealer bias: < 1/N probability (N = # validators)
- Unpredictable until majority reveals
- Slashable if commitment != hash(secret)

### Trust Dynamics

**Initial State:**
```rust
trust_q = 1.0 (ONE_Q = 1u64 << 32)
```

**Update Rules:**

1. **Successful Block Proposal:**
   ```rust
   trust_new = min(trust × 1.01, max_trust_q)
   ```

2. **Missed Slot (Timeout):**
   ```rust
   trust_new = max(trust × 0.90, min_trust_q)
   ```

3. **Equivocation (Double Proposal):**
   ```rust
   trust_new = 0  // Immediate slash
   stake_q = 0     // Forfeit all stake
   ```

**Convergence:**
- Honest validators → trust ≈ 1.0-2.0
- Unreliable validators → trust ≈ 0.01-0.1
- Effect on selection: 100x difference in leader probability

### Epoch Snapshots

**Snapshot Creation (every 32 slots):**

```rust
struct EpochSnapshot {
    epoch: u64,
    weights_root: [u8; 32],           // Merkle root
    weights: HashMap<NodeId, Q>,      // stake×trust for each validator
    total_weight_q: u128,             // Σweights
}

fn build_snapshot(registry: &Registry, trust: &TrustState) -> EpochSnapshot {
    let mut leaves = Vec::new();
    for (node_id, info) in registry.validators {
        let trust_q = trust.get(node_id);
        let weight = qmul(info.stake, trust_q);
        let leaf = merkle_leaf_hash(node_id, info.stake, trust_q);
        leaves.push(leaf);
    }
    
    let root = build_merkle_tree(leaves);
    EpochSnapshot { epoch, weights_root: root, weights, total_weight_q }
}
```

**Merkle Tree Structure:**
- Leaf: `SHA256("WGT.v1" || node_id || stake_q || trust_q)`
- Parent: `SHA256("MRK.v1" || left || right)`
- Height: log2(N) where N = # validators

**Purpose:**
- Compact commitment to validator set
- Efficient proof of inclusion (log N)
- Prevents retroactive weight changes

---

## Layer 2: PoZS zkSNARK

### Groth16 Circuit

**Public Inputs (4 field elements):**
```rust
pub struct EligibilityPublicInputs {
    weights_root: [u8; 32],    // Merkle root from snapshot
    beacon_value: [u8; 32],    // RANDAO beacon for slot
    threshold_q: u64,          // Computed eligibility threshold
    sum_weights_q: u128,       // Total validator weight
}
```

**Private Witness:**
```rust
pub struct EligibilityWitness {
    who: NodeId,               // Validator ID (secret!)
    slot: u64,                 // Current slot
    stake_q: u128,             // Validator stake
    trust_q: u64,              // Trust score
    merkle_siblings: Vec<[u8; 32]>, // Merkle proof path
    leaf_index: u64,           // Position in tree
}
```

**Constraint System (R1CS):**

1. **Merkle Path Verification:**
   ```
   Reconstruct root from (who, stake_q, trust_q) + siblings
   Assert: computed_root == public_weights_root
   ```

2. **Weight Computation:**
   ```
   weight = stake_q × trust_q
   Assert: weight × sum_weights >= threshold × total_weight
   ```

3. **Eligibility Hash:**
   ```
   elig = Poseidon(beacon || slot || who)  // Poseidon for ZK-friendliness
   Assert: elig < threshold_bound
   ```

**Proof Size:**
- Groth16: 128 bytes (2 G1 points + 1 G2 point on BN254)
- Compare to classical Merkle proof: ~32 × log2(N) bytes

**Performance:**
- Setup: ~1-5 seconds (one-time, reusable for all proofs)
- Prove: ~0.5-2 seconds per block
- Verify: <10ms

**Security:**
- Computational soundness: 128-bit (BN254 curve)
- Zero-knowledge: Hides validator identity
- Non-interactive: No back-and-forth

### Why Groth16 over Other Systems?

| System | Proof Size | Verify Time | Setup | Notes |
|--------|-----------|-------------|-------|-------|
| **Groth16** | 128 B | 5-10ms | Trusted | ✅ Chosen (smallest proofs) |
| PLONK | 512 B | 20-30ms | Universal | Better for updates |
| Halo2 | 256 B | 15-25ms | Transparent | No trusted setup |
| STARKs | 100-300 KB | 50-100ms | Transparent | Post-quantum! |

**Decision:** Groth16 for production due to:
- Proven track record (Zcash, Filecoin)
- Ethereum compatibility (BN254)
- Smallest proof size → bandwidth efficient
- Fast verification → node scalability

**Future:** Consider STARKs for post-quantum soundness.

---

## Layer 3: Post-Quantum Wallet

### Key Generation

**Falcon512 (Signatures):**
```rust
let (falcon_pk, falcon_sk) = pqcrypto_falcon::falcon512::keypair();
// pk: ~897 bytes
// sk: ~1281 bytes
// sig: ~666 bytes
```

**ML-KEM-768 (KEM):**
```rust
let (kyber_pk, kyber_sk) = pqcrypto_kyber::kyber768::keypair();
// pk: 1184 bytes
// sk: 2400 bytes
// ciphertext: 1088 bytes
```

**Node ID Derivation:**
```rust
node_id = SHA256("NODE_ID.v1" || falcon_pk)[..32]
```

### Wallet Storage Format

**Encryption Pipeline:**
```
Password
  ↓ [Argon2id: 64MB, 3 iterations]
Key (32 bytes)
  ↓ [XChaCha20-Poly1305 AEAD]
Ciphertext + Tag
  ↓ [Bincode serialization]
wallet.enc file
```

**File Structure:**
```rust
struct EncryptedWallet {
    version: u32,           // 1
    salt: Vec<u8>,         // 32 bytes (random)
    nonce: Vec<u8>,        // 24 bytes (XChaCha20)
    ciphertext: Vec<u8>,   // Encrypted KeyExport
}

struct KeyExport {
    falcon_pk: Vec<u8>,    // ~897 B
    falcon_sk: Vec<u8>,    // ~1281 B (sensitive!)
    kyber_pk: Vec<u8>,     // 1184 B
    kyber_sk: Vec<u8>,     // 2400 B (sensitive!)
}
```

**Security Properties:**
- Password never stored
- Argon2id: memory-hard (64MB), resistant to GPUs/ASICs
- XChaCha20-Poly1305: 192-bit nonce (no reuse concerns)
- Zeroizing: Clears sensitive data on drop
- File permissions: 0600 (user read/write only)

### Atomic File Operations

**Race-Free Wallet Creation:**
```rust
fn create_pepper() -> Result<[u8; 32]> {
    // Try to read existing
    if let Ok(data) = fs::read(&path) {
        return Ok(data);
    }
    
    // Create new with create_new(true) - atomic!
    let pepper = random_bytes(32);
    let mut opts = OpenOptions::new();
    opts.create_new(true).write(true).mode(0o600);
    
    match opts.open(&path) {
        Ok(mut f) => {
            f.write_all(&pepper)?;
            Ok(pepper)
        }
        Err(AlreadyExists) => {
            // Another process created it - read theirs
            fs::read(&path)
        }
        Err(e) => Err(e),
    }
}
```

**Robust File Replacement:**
```rust
fn atomic_replace(path: &Path, data: &[u8]) -> Result<()> {
    let tmp = path.with_extension(".tmp.{PID}");
    
    // Write to temp
    fs::write(&tmp, data)?;
    
    // Atomic rename (POSIX guarantee)
    #[cfg(unix)]
    fs::rename(&tmp, path)?;
    
    // Windows: backup + rename + remove backup
    #[cfg(windows)]
    {
        let backup = path.with_extension(".bak");
        if path.exists() {
            fs::rename(path, &backup)?;
        }
        match fs::rename(&tmp, path) {
            Ok(_) => { let _ = fs::remove_file(&backup); }
            Err(e) => {
                if backup.exists() {
                    fs::rename(&backup, path)?; // Rollback!
                }
                return Err(e.into());
            }
        }
    }
    
    Ok(())
}
```

---

## Networking Layer

### P2P Protocol

**Message Types:**
```rust
enum NetworkMessage {
    NewBlock(Block),
    BlockRequest(u64),
    BlockResponse(Option<Block>),
    RandaoCommit(Proposal),
    RandaoReveal(Proposal),
    Ping,
    Pong,
    PeerList(Vec<String>),
    SyncRequest { start_slot: u64, end_slot: u64 },
    SyncResponse { blocks: Vec<Block> },
}
```

**Encoding:**
- Length-delimited frames (4-byte prefix)
- Bincode serialization
- Max message size: 16 MB

**Gossip Algorithm:**
```rust
fn gossip_block(block: Block) {
    let slot = block.slot;
    
    // Deduplication
    if seen_blocks.contains(slot) {
        return;
    }
    seen_blocks.insert(slot);
    
    // Broadcast to all connected peers
    for peer in peers {
        peer.send(NetworkMessage::NewBlock(block.clone()));
    }
}
```

**Connection Management:**
- TCP with Tokio async runtime
- One task per connection
- Automatic reconnection to bootstrap peers
- Keep-alive pings (TODO)

**Security (TODO):**
- [ ] Noise protocol handshake
- [ ] Peer reputation system
- [ ] Rate limiting per peer
- [ ] Eclipse attack prevention

---

## Storage Layer

### Sled Database

**Key-Value Schema:**

```
Blocks:
  "block:{slot}" → bincode(Block)

Snapshots:
  "snapshot:{epoch}" → bincode(EpochSnapshot)

State (future):
  "state:{root}" → bincode(StateTree)
  "account:{address}" → bincode(Account)
```

**Operations:**
```rust
// Write
storage.store_block(&block)?;
storage.db.flush()?; // Ensure durability

// Read
let block = storage.get_block(slot)?;

// Range query
let blocks: Vec<Block> = storage.db
    .scan_prefix(b"block:")
    .map(|r| bincode::deserialize(&r?.1))
    .collect()?;
```

**Properties:**
- Embedded (no separate server)
- ACID transactions
- Crash-safe (WAL)
- ~100K writes/sec on SSD

---

## Performance Analysis

### Consensus Throughput

**Assumptions:**
- 1000 validators
- 32 slots/epoch
- 6 sec/slot
- 10% leader ratio

**Calculations:**
```
Block time: 6 seconds
Max leaders per slot: λ × N = 0.1 × 1000 = 100
Effective leaders: ~1-3 (probabilistic)
Blocks per hour: 600
TPS (if 1000 txs/block): ~167 TPS
```

**Bottlenecks:**
1. Signature verification: ~1ms/sig × 1000 txs = 1 sec
2. State transition: ~0.5 sec (depends on VM)
3. Gossip propagation: ~0.5 sec (10 hops @ 50ms/hop)
4. ZK verification: ~0.01 sec

**Total:** ~2 sec per block → 4 sec buffer for finalization.

### Memory Usage

**Per Validator:**
```
Wallet keys:       ~5 KB
Trust state:       8 bytes × N = 8 KB (1000 validators)
Epoch snapshot:    32 bytes × N = 32 KB
Block buffer:      ~1 MB (recent blocks)
Sled cache:        ~100 MB (configurable)
---
Total:             ~102 MB
```

**With ZK (feature enabled):**
```
Proving key:       ~50 MB
Verifying key:     ~1 MB
Constraint sys:    ~20 MB
---
Additional:        ~71 MB
```

**Total (ZK enabled):** ~173 MB/validator.

### Network Bandwidth

**Per Block:**
```
Header:            ~200 bytes
Transactions:      ~500 bytes × 1000 = 500 KB
Signatures:        ~666 bytes × 1000 = 666 KB
ZK proof (opt):    128 bytes
---
Total:             ~1.2 MB/block
```

**Per Hour:**
```
600 blocks × 1.2 MB = 720 MB/hour
= 200 KB/sec average
= 1.6 Mbps
```

**Gossip amplification:** 2-3× (redundant sends) → **3-5 Mbps** sustained.

---

## Security Model

### Threat Model

**Assumptions:**
- Byzantine adversary controls ≤ 1/3 of total weight
- Adversary cannot break:
  - Falcon512 (quantum attacker with <2^128 queries)
  - ML-KEM-768 (quantum attacker with <2^128 queries)
  - SHA3-512 (preimage, collision resistance)
  - Groth16 (computational soundness on BN254)

**Attack Vectors:**

1. **Grinding Attack:**
   - Try different RANDAO reveals to maximize future leader slots
   - Mitigation: Commit-reveal → last revealer bias only
   - Cost: Lose current slot rewards

2. **Nothing-at-Stake:**
   - Build on multiple forks
   - Mitigation: Trust decay for equivocation
   - Penalty: Immediate slash to trust=0, stake=0

3. **Long-Range Attack:**
   - Rewrite history from old snapshot
   - Mitigation: Finality gadget (TODO), checkpoints

4. **Eclipse Attack:**
   - Isolate victim node from honest peers
   - Mitigation: Diverse peer discovery (TODO)

5. **DDoS:**
   - Flood leader with invalid blocks
   - Mitigation: Rate limiting, reputation (TODO)

### Safety Guarantees

**Theorem (Informal):**
If adversary controls < 1/3 total weight, then:
1. **Safety:** No two conflicting blocks finalized
2. **Liveness:** At least one block per epoch
3. **Fairness:** Leader selection probability ∝ weight

**Proof Sketch:**
- Safety: 2/3 honest votes required for finalization
- Liveness: Expected honest leaders = 2/3 × λ × N × 32 > 1
- Fairness: Direct from eligibility hash distribution

---

## Future Work

### Near-Term (Q1-Q2 2025)

1. **Full Keccak Gadget**
   - Implement Keccak-f[1600] permutation in R1CS
   - ~100K constraints (feasible for Groth16)

2. **Signature Verification**
   - Add Falcon512 verification to block validation
   - Batch verification for efficiency

3. **Finality Gadget**
   - GRANDPA-style voting (2/3 supermajority)
   - ~10 epoch finalization delay

4. **P2P Security**
   - Noise protocol handshake
   - Peer authentication with PQ keys

### Mid-Term (Q3-Q4 2025)

5. **Light Clients**
   - Sync committee (Ethereum-style)
   - Merkle proofs for state queries

6. **Cross-Chain Bridges**
   - IBC-style relayer
   - ZK proofs for remote state

7. **Smart Contracts**
   - WASM VM (Wasmer)
   - Gas metering

### Long-Term (2026+)

8. **Sharding**
   - Data availability sampling
   - Cross-shard communication

9. **STARK Migration**
   - Post-quantum soundness
   - Recursive proof composition

10. **Formal Verification**
    - TLA+ spec for consensus
    - Coq proofs for crypto

---

## Appendix: Q32.32 Fixed-Point Arithmetic

**Representation:**
```
Q = u64 where Q = (integer_part << 32) | fractional_part

Examples:
  1.0   = 0x0000000100000000 = 2^32
  0.5   = 0x0000000080000000 = 2^31
  1.25  = 0x0000000140000000 = 2^32 + 2^30
  0.01  = 0x00000000028F5C29 ≈ 42949673
```

**Operations:**
```rust
// Multiply: (a × b) >> 32
fn qmul(a: Q, b: Q) -> Q {
    ((a as u128 * b as u128) >> 32) as Q
}

// Divide: (a << 32) / b
fn qdiv(a: Q, b: Q) -> Option<Q> {
    if b == 0 { return None; }
    Some((((a as u128) << 32) / b as u128) as Q)
}

// Clamp to [0, 1]
fn qclamp01(q: Q) -> Q {
    if q > ONE_Q { ONE_Q } else { q }
}
```

**Precision:**
- Integer part: 32 bits (0 to 4,294,967,295)
- Fractional part: 32 bits (~2.3 × 10^-10 resolution)
- Good for: 0.0001 to 1,000,000 range

**Why Q32.32?**
- No floating-point non-determinism
- Exact representation of common fractions (0.5, 0.25, etc.)
- Efficient on 64-bit CPUs
- Avoids `f64` rounding issues in consensus

---

**End of Architecture Document**
