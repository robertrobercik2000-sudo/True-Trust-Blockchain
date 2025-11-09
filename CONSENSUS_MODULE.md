# Consensus Module - Proof-of-Trust (PoT)

## Overview

This module implements a sophisticated Proof-of-Trust (PoT) consensus mechanism with RANDAO beacon integration, Merkle proof verification, and economic incentives/penalties.

## Architecture

### Core Components

#### 1. Q32.32 Fixed-Point Arithmetic
- **Type**: `Q = u64` representing 32.32 fixed-point numbers
- **Range**: [0, 1] for probabilities and ratios
- **Operations**: `qmul`, `qadd`, `qdiv`, `qclamp01`
- **Conversions**: `q_from_ratio`, `q_from_ratio128`, `q_from_basis_points`

#### 2. Trust System
- **Parameters**: `TrustParams { alpha_q, beta_q, init_q }`
  - `alpha_q`: Decay factor (e.g., 0.99 = 99% retention)
  - `beta_q`: Reward increment (e.g., 0.01 = 1% boost)
  - `init_q`: Initial trust for new nodes
- **Update Rule**: `trust' = min(1.0, alpha * trust + beta)`
- **State**: `TrustState` - HashMap tracking per-node trust values

#### 3. Registry (Stake & Active Status)
- **Entry**: `RegEntry { who: NodeId, stake: u64, active: bool }`
- **Operations**:
  - `is_active(who, min_bond)` - Check if node qualifies
  - `stake(who)` - Query stake amount
  - `stake_mut(who)` - Mutable stake access for slashing

#### 4. Epoch Snapshot (Deterministic Merkle Tree)
- **Purpose**: Immutable weight distribution snapshot per epoch
- **Contents**:
  - `sum_weights_q`: Σ(stake_q × trust_q) - total network weight
  - `stake_q`: Normalized stake per node (∈ [0,1])
  - `trust_q_at_snapshot`: Trust values frozen at snapshot time
  - `order`: Deterministic leaf ordering (sorted by NodeId)
  - `weights_root`: Merkle root of weight tree

- **Construction**:
  ```rust
  EpochSnapshot::build(epoch, registry, trust, params, min_bond)
  ```

- **Merkle Proof**:
  ```rust
  snapshot.build_proof(&node_id) -> Option<MerkleProof>
  snapshot.verify_witness(&witness) -> bool
  ```

#### 5. RANDAO Beacon
- **Commit-Reveal Scheme**:
  1. Validators commit to random values: `commit_hash(epoch, who, r)`
  2. Later reveal the preimage `r`
  3. Beacon = sequential mixing of all reveals
  
- **Seed Stability**: 
  - Each epoch uses `prev_beacon` as stable seed
  - New beacon from reveals becomes `prev_beacon` for next epoch
  - **Critical**: Sortition seed is stable before/after finalization

- **Slashing**: 
  - Nodes that commit but don't reveal are slashed
  - Penalty: `slash_noreveal_bps` basis points

- **API**:
  ```rust
  beacon.commit(epoch, who, commitment)
  beacon.reveal(epoch, who, preimage) -> bool
  beacon.finalize_epoch(epoch) -> (beacon, missing_nodes)
  beacon.value(epoch, slot) -> [u8; 32]  // stable seed
  ```

#### 6. Sortition (Leader Election)
- **Eligibility Test**:
  ```
  p = λ × (stake_q × trust_q) / sum_weights_q
  y = hash(beacon, slot, who)
  eligible iff y ≤ bound(p)
  ```

- **Block Weight** (fork-choice):
  ```
  weight = 2^64 / (y + 1)
  ```
  Lower `y` → higher weight → preferred chain

- **Verification Functions**:
  - `verify_leader_and_update_trust()` - Classic API with explicit MerkleProof
  - `verify_leader_with_witness()` - Compact API using `WeightWitnessV1`

#### 7. Equivocation Detection & Slashing
- **Detection**: 
  ```rust
  detect_equivocation(proposals) -> bool
  // Returns true if same (slot, who) has multiple header_hashes
  ```

- **Penalty**:
  ```rust
  slash_equivocation(registry, trust, who, params, penalty_bps)
  // Resets trust to init_q, slashes stake by penalty_bps
  ```

### Witness System (snapshot.rs)

#### WeightWitnessV1 (Compact Proof)
```rust
pub struct WeightWitnessV1 {
    pub who: NodeId,
    pub stake_q: StakeQ,
    pub trust_q: Q,
    pub leaf_index: u64,
    pub siblings: Vec<[u8; 32]>,  // Merkle path
}
```

**Advantages**:
- Self-contained proof of weight distribution
- Verifiable against snapshot root without full snapshot data
- Efficient for light clients

**Verification**:
```rust
impl EpochSnapshot {
    fn verify_witness(&self, wit: &WeightWitnessV1) -> bool {
        // 1. Check stake_q and trust_q match snapshot
        // 2. Verify leaf_index corresponds to 'who'
        // 3. Verify Merkle proof against weights_root
    }
}
```

## Security Properties

### 1. Byzantine Fault Tolerance
- Trust decay prevents sustained trust from malicious actors
- Equivocation detection catches double-signing
- Slashing provides economic deterrent

### 2. Sybil Resistance
- Stake-weighted probabilities (stake_q component)
- Trust accumulation requires consistent participation
- Linear relationship: weight ∝ stake × trust

### 3. Censorship Resistance
- Multiple potential leaders per slot (probabilistic)
- Higher trust/stake → higher probability, not exclusivity
- RANDAO prevents predictable leader schedules

### 4. Randomness Quality
- RANDAO beacon combines multiple independent reveals
- Commit-reveal prevents last-revealer bias
- Deterministic seed derivation per slot

### 5. Merkle Proof Security
- Deterministic leaf ordering prevents reordering attacks
- SHA-256 based (collision-resistant)
- Domain separation: `"WGT.v1"` (leaf), `"MRK.v1"` (internal)

## Economic Model

### Incentives
1. **Block Rewards**: `trust' = step(trust)` after successful block
2. **Trust Accumulation**: Consistent participation → higher eligibility
3. **Weight Advantage**: Higher trust → better fork-choice weight

### Penalties
1. **No Reveal**: `-slash_noreveal_bps` stake, trust → init_q
2. **Equivocation**: `-penalty_bps` stake, trust → init_q
3. **Inactivity**: Trust decays by factor `alpha_q` (implicit)

### Parameter Tuning
```rust
// Example: Conservative parameters
TrustParams {
    alpha_q: q_from_basis_points(9900),  // 99% retention
    beta_q: q_from_basis_points(100),    // 1% reward
    init_q: q_from_basis_points(1000),   // 10% initial
}

PotParams {
    trust: trust_params,
    lambda_q: q_from_ratio(11, 10),      // 1.1 expected leaders/slot
    min_bond: 1_000_000,                 // 1M minimum stake
    slash_noreveal_bps: 500,             // 5% penalty
}
```

## Usage Examples

### 1. Initialize System
```rust
let mut registry = Registry::default();
let mut trust = TrustState::default();
let mut beacon = RandaoBeacon::new(500, genesis_beacon);

let trust_params = TrustParams {
    alpha_q: q_from_basis_points(9900),
    beta_q: q_from_basis_points(100),
    init_q: q_from_basis_points(1000),
};

// Register validators
registry.insert(node_id, 1_000_000, true);
```

### 2. Start New Epoch
```rust
// Create snapshot
let snapshot = EpochSnapshot::build(
    epoch,
    &registry,
    &trust,
    &trust_params,
    min_bond
);

// Validators commit to randomness
let r = rand::random();
let c = RandaoBeacon::commit_hash(epoch, &node_id, &r);
beacon.commit(epoch, node_id, c);
```

### 3. Verify Block Leader
```rust
// Validator reveals randomness
beacon.reveal(epoch, node_id, r);

// Build witness for block production
let witness = snapshot.build_witness(&node_id).unwrap();

// Verify eligibility
let weight = verify_leader_with_witness(
    &registry,
    &snapshot,
    &beacon,
    &mut trust,
    &params,
    epoch,
    slot,
    &witness
).expect("valid leader");
```

### 4. Finalize Epoch
```rust
// Finalize beacon, slash non-revealers
let beacon_value = finalize_epoch_and_slash(
    &mut beacon,
    epoch,
    &mut registry,
    &mut trust,
    trust_params
);
```

### 5. Handle Equivocation
```rust
// Detect double-signing
let proposals = vec![
    Proposal { who: attacker, slot: 42, header_hash: hash1 },
    Proposal { who: attacker, slot: 42, header_hash: hash2 },
];

if detect_equivocation(&proposals) {
    slash_equivocation(&mut registry, &mut trust, &attacker, trust_params, 5000);
    // 50% stake penalty, trust reset
}
```

## Testing

The module includes comprehensive tests:

```bash
cd pot80_zk_host
cargo test consensus::tests
cargo test snapshot::tests
```

**Test Coverage**:
- ✅ Probability monotonicity
- ✅ RANDAO commit-reveal flow
- ✅ Snapshot determinism
- ✅ Merkle proof verification
- ✅ Large stake handling (u128 arithmetic)
- ✅ Beacon stability across finalization
- ✅ Witness roundtrip (build + verify)

## Performance Considerations

### Scalability
- **Snapshot Creation**: O(N log N) - sorting nodes
- **Merkle Proof**: O(log N) - tree height
- **Verification**: O(log N) - proof path length
- **RANDAO Mixing**: O(N) - iterate reveals

### Optimizations
1. **Lazy Evaluation**: Only build proofs when needed
2. **Caching**: Reuse snapshot across same epoch
3. **Batch Verification**: Verify multiple witnesses in parallel
4. **Compact Witnesses**: ~32 × log₂(N) bytes per proof

## Future Enhancements

1. **VRF Integration**: Replace hash-based sortition with VRF for unpredictability proofs
2. **Finality Gadget**: Add BFT finality on top of longest-chain
3. **Adaptive Parameters**: Auto-tune λ based on network conditions
4. **Slashing Appeals**: Time-locked stake recovery for false positives
5. **Cross-Shard Trust**: Propagate trust scores across shards

## References

- Fixed-point arithmetic: Q32.32 format
- RANDAO: Ethereum beacon chain specification
- Merkle proofs: RFC 6962 (Certificate Transparency)
- Trust systems: EigenTrust, PageRank adaptations

---

**Status**: ✅ Implemented and tested  
**Dependencies**: `sha2`, `std::collections`  
**Safety**: `#![forbid(unsafe_code)]`
