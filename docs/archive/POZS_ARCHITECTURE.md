# PoZS Integration Architecture

## 🎯 Overview

This document describes the integration of **PoZS (Proof-of-ZK-Shares)** with the existing **PoT (Proof-of-Trust)** consensus mechanism. The integration is designed as a **complementary verification layer** that enhances security without replacing the core consensus logic.

---

## 🏗️ System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    TRUE_TRUST Consensus                      │
│                                                               │
│  ┌────────────┐      ┌────────────┐      ┌────────────┐    │
│  │   RANDAO   │──────│    PoT     │──────│  PoZS (ZK) │    │
│  │   Beacon   │      │  Sortition │      │   Proofs   │    │
│  └────────────┘      └────────────┘      └────────────┘    │
│        │                    │                    │           │
│        └────────────────────┴────────────────────┘           │
│                            │                                 │
│                   ┌────────▼────────┐                       │
│                   │ Hybrid Verifier │                       │
│                   └─────────────────┘                       │
└─────────────────────────────────────────────────────────────┘
```

---

## 📊 Current PoT Consensus (pot.rs)

### Key Components

1. **Trust System** (`TrustState`)
   - Dynamic trust scores: `trust_q ∈ [0, 1]` (Q32.32 fixed-point)
   - Decay: `trust' = α × trust` (α ≈ 0.99)
   - Reward: `trust' = min(trust + β, 1)` (β ≈ 0.01)

2. **Stake Registry** (`Registry`)
   - Bonded stake per validator
   - Active/inactive status
   - Minimum bond enforcement

3. **Epoch Snapshots** (`EpochSnapshot`)
   - Merkle tree of weights: `weight = stake_q × trust_q`
   - Deterministic ordering (sorted by NodeId)
   - SHA256-based Merkle proofs

4. **RANDAO Beacon** (`RandaoBeacon`)
   - Commit-reveal scheme per epoch
   - Entropy: `beacon[e] = mix(beacon[e-1], reveals[e])`
   - Slashing for non-reveals

5. **Leader Selection** (probabilistic sortition)
   ```rust
   threshold = λ × (stake_q × trust_q) / Σweights
   eligible = hash(beacon || slot || who) < bound(threshold)
   weight = (2^64 - 1) / (hash + 1)  // Lower hash → higher weight
   ```

---

## 🔐 PoZS Enhancement (pozs.rs)

### What PoZS Adds

PoZS provides **cryptographic proofs** of leader eligibility, enabling:

1. **Lighter Verification**
   - Full nodes verify **1 pairing** (Groth16) instead of **Merkle path + hash**
   - ~10ms verification vs ~50ms classical path

2. **Privacy**
   - Hides exact `stake_q` and `trust_q` from block headers
   - Only Merkle root + proof published

3. **Recursive Aggregation** (future)
   - Nova/Halo2 folding of multiple blocks
   - Sync nodes verify **1 proof** for **N blocks**

### ZK Circuit (Groth16/BN254)

```
Public Inputs:
  - weights_root: [u8; 32]      // Merkle root of epoch snapshot
  - beacon_value: [u8; 32]      // RANDAO seed for (epoch, slot)
  - threshold_q: Q              // Eligibility threshold

Private Inputs:
  - who: NodeId                 // Validator identity
  - stake_q: Q                  // Normalized stake
  - trust_q: Q                  // Trust score
  - merkle_path: Vec<[u8; 32]>  // Siblings for Merkle proof

Constraints:
  1. Merkle verification:
     root = recompute_merkle(hash(who || stake_q || trust_q), merkle_path)
  
  2. Eligibility check:
     y = hash(beacon || slot || who)
     y < bound(threshold_q)
  
  3. Threshold computation:
     threshold = λ × (stake_q × trust_q) / Σweights
```

---

## 🔀 Hybrid Verification Path

### Classical PoT (existing)

```rust
pub fn verify_leader_with_witness(
    reg: &Registry,
    epoch_snap: &EpochSnapshot,
    beacon: &RandaoBeacon,
    trust_state: &mut TrustState,
    params: &PotParams,
    epoch: u64,
    slot: u64,
    wit: &WeightWitnessV1,  // Merkle witness
) -> Option<u128> { ... }
```

### PoZS-Enhanced (new)

```rust
pub fn verify_leader_zk(
    reg: &Registry,
    epoch_snap: &EpochSnapshot,
    beacon: &RandaoBeacon,
    trust_state: &mut TrustState,
    params: &PotParams,
    witness: &ZkLeaderWitness,  // Merkle OR ZK proof
    verifier: Option<&ZkVerifier>,
) -> Result<u128, ZkError> {
    // 1. Classical verification (always)
    let weight = verify_leader_classical(...)?;
    
    // 2. ZK verification (if proof present)
    if let (Some(proof), Some(ver)) = (&witness.zk_proof, verifier) {
        verify_zk_proof(proof, beacon, weights_root, threshold)?;
    }
    
    // 3. Update trust (reward)
    trust_state.apply_block_reward(&witness.who, params.trust);
    
    Ok(weight)
}
```

---

## 📦 Data Structures

### ZkLeaderWitness (extended LeaderWitness)

```rust
pub struct ZkLeaderWitness {
    pub who: NodeId,
    pub slot: u64,
    pub epoch: u64,
    pub weights_root: [u8; 32],
    
    // Backward-compatible: can use Merkle OR ZK
    pub merkle_proof: Option<Vec<u8>>,   // Classical path
    pub zk_proof: Option<ZkProof>,       // PoZS enhancement
    
    // Validator data (used in classical verification)
    pub stake_q: Q,
    pub trust_q: Q,
}
```

### ZkProof

```rust
pub struct ZkProof {
    pub scheme: ZkScheme,       // Groth16BN254 | PlonkBLS12 | NovaRecursive
    pub proof_bytes: Vec<u8>,   // ~200 bytes (Groth16)
}
```

---

## 🚀 Integration Points

### 1. Block Producer (Validator Node)

```rust
// In pot_node.rs or custom validator

let prover = ZkProver::new(ZkScheme::Groth16BN254)?;

// After winning sortition
if i_am_eligible(epoch, slot) {
    let beacon = beacon.value(epoch, slot);
    let proof = prover.prove_eligibility(
        &beacon,
        slot,
        &my_node_id,
        my_stake_q,
        my_trust_q,
        threshold_q,
    )?;
    
    let witness = ZkLeaderWitness {
        who: my_node_id,
        slot,
        epoch,
        weights_root: snapshot.weights_root,
        merkle_proof: None,  // Can skip classical proof
        zk_proof: Some(proof),
        stake_q: my_stake_q,
        trust_q: my_trust_q,
    };
    
    broadcast_block_with_witness(witness);
}
```

### 2. Block Verifier (Full Node)

```rust
// In pot_node.rs

let verifier = ZkVerifier::new(ZkScheme::Groth16BN254)?;

for block in new_blocks {
    let weight = verify_leader_zk(
        &registry,
        &epoch_snapshot,
        &beacon,
        &mut trust_state,
        &params,
        &block.witness,
        Some(&verifier),  // Enable ZK verification
    )?;
    
    apply_block(block, weight)?;
}
```

### 3. Hybrid Node (supports both)

```rust
// Accepts BOTH Merkle and ZK proofs

match &witness.zk_proof {
    Some(proof) => {
        // Fast path: ZK verification (~10ms)
        verify_leader_zk(..., Some(&verifier))?
    }
    None => {
        // Fallback: classical Merkle verification (~50ms)
        verify_leader_with_witness(...)?
    }
}
```

---

## 🔄 Migration Strategy

### Phase 1: Soft Deployment (optional ZK)
- Classical verification remains mandatory
- ZK proofs are **optional** and verified **in addition to** Merkle
- Backward-compatible with existing nodes

### Phase 2: Hybrid Mode (dual-path)
- Nodes can choose verification method
- ZK-capable nodes prefer faster verification
- Legacy nodes continue using Merkle

### Phase 3: ZK-Primary (future)
- ZK proofs become standard
- Merkle proofs deprecated
- Recursive aggregation enabled (Nova/Halo2)

---

## 📈 Performance Targets

| Metric                  | Classical (Merkle) | PoZS (Groth16) | PoZS (Nova Aggregated) |
|-------------------------|--------------------|-----------------|-----------------------|
| Proof Size              | ~1 KB              | ~200 bytes      | ~200 bytes            |
| Proof Generation Time   | N/A                | <500 ms         | <2s (fold 100 blocks) |
| Verification Time       | ~50 ms             | ~10 ms          | ~10 ms (100 blocks!)  |
| Privacy                 | ❌ Public data     | ✅ Hidden data   | ✅ Hidden data         |
| Aggregation             | ❌ No              | ❌ No            | ✅ Recursive folding   |

---

## 🛠️ Production Checklist

### To Deploy PoZS:

- [ ] **Circuit Implementation** (arkworks/halo2)
  - [ ] Poseidon hash gadgets
  - [ ] Merkle path verification
  - [ ] Threshold comparison
  - [ ] Eligibility hash computation

- [ ] **Trusted Setup** (Groth16) or Universal Setup (PLONK)
  - [ ] Run MPC ceremony or load universal SRS
  - [ ] Embed verifying key in binary

- [ ] **Prover Infrastructure**
  - [ ] Proving key caching on validator nodes
  - [ ] GPU acceleration (optional)
  - [ ] Batch proving for multiple slots

- [ ] **Integration**
  - [ ] Extend `SlotDecision` in pot_node.rs
  - [ ] Serialize `ZkLeaderWitness` in block headers
  - [ ] Add `--zk-mode` CLI flag for validators

- [ ] **Testing**
  - [ ] Circuit soundness tests
  - [ ] Fuzz testing for proof verification
  - [ ] Load testing (1000+ blocks/sec)

- [ ] **Monitoring**
  - [ ] Proof generation latency metrics
  - [ ] Verification failure rates
  - [ ] Proof size distribution

---

## 📚 References

### Code Structure

```
src/
├── crypto_kmac_consensus.rs  (100 lines)  - KMAC256 utilities
├── pot.rs                    (765 lines)  - PoT consensus core
├── pot_node.rs               (481 lines)  - Validator node runtime
├── pozs.rs                   (460 lines)  - ⭐ PoZS ZK integration
├── snapshot.rs               (162 lines)  - Merkle witness verification
└── main.rs                  (1122 lines)  - Wallet CLI (separate)
```

### Key Exports

```rust
// From pozs.rs
pub use pozs::{
    ZkProver,           // Proof generation
    ZkVerifier,         // Proof verification  
    ZkLeaderWitness,    // Extended witness format
    ZkProof,            // Serialized proof
    ZkScheme,           // Groth16 | PLONK | Nova
    verify_leader_zk,   // Hybrid verification function
    AggregatedProof,    // Recursive aggregation (future)
};
```

---

## 🎯 Summary

**PoZS is NOT a replacement** for PoT consensus — it's a **cryptographic enhancement layer** that:

1. ✅ Preserves existing PoT logic (RANDAO + trust + stake)
2. ✅ Adds optional ZK proofs for faster/private verification
3. ✅ Enables recursive aggregation (future)
4. ✅ Backward-compatible with classical Merkle witnesses

The hybrid approach allows gradual adoption while maintaining full compatibility with existing infrastructure.

---

## 📞 Contact & Contribution

For questions about PoZS integration:
- Review: `src/pozs.rs` (460 lines, well-documented stubs)
- Tests: `cargo test --lib pozs` (5 passing tests)
- Architecture: This document

**Current Status**: 🟡 **Stub Implementation** (proof generation/verification are placeholders)

**Production Deployment**: Requires circuit implementation with arkworks or halo2.

---

*Generated: 2025-11-10*  
*Project: TRUE_TRUST Proof-of-Trust Consensus v5.0*
