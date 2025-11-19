# Consensus Model

## Formula

```
Weight = 4·Trust + 2·Quality + 1·Stake

Trust T(v) = S(β₁·H + β₂·V + β₃·W)
  H = historical quality (EWMA, α=0.99)
  V = vouching (capped at 1.0)
  W = current work quality
  S(x) = 3x² − 2x³ (S-curve)

β₁ = 0.4 (history)
β₂ = 0.3 (vouching)
β₃ = 0.3 (work)
```

## Leader Selection

```rust
leader = argmax(SHA3(beacon || slot || validator_id) * weight)
```

Deterministic. No randomness after beacon.

## Implementation

- **rtt_pro.rs**: Q32.32 fixed-point (no floats)
- **consensus_weights.rs**: Integer weight calculation
- **consensus_pro_v2.rs**: Validator state management

## Status

- ✅ Algorithm implemented
- ❌ No actual blockchain integration
- ❌ Untested at scale
- ❌ No formal proof of safety/liveness
