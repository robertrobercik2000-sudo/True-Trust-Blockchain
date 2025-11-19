# Architecture

## Components

```
CLI Wallet (tt_priv_cli)
  ├─ Falcon512 + Kyber768
  ├─ Argon2id KDF
  └─ Shamir secret sharing

Node (tt_node) - INCOMPLETE
  ├─ Consensus (basic structure)
  ├─ P2P (channel crypto only)
  └─ State (not implemented)

Library (tt_priv_cli)
  ├─ src/rtt_pro.rs         - Trust algorithm (Q32.32)
  ├─ src/consensus_*.rs     - Weight calculation
  ├─ src/p2p_channel.rs     - Secure channel
  ├─ src/stark_*.rs         - ZK proofs (unoptimized)
  ├─ src/falcon_sigs.rs     - PQ signatures
  ├─ src/kyber_kem.rs       - PQ KEM
  └─ src/crypto/            - KMAC, DRBG
```

## Data Flow (Theoretical)

```
Transaction → STARK proof → Encrypt (Kyber) → Broadcast
Block → Verify (Falcon) → Verify (STARK) → Update state
Consensus → RTT trust → Weights → Leader selection
```

## Not Implemented

- Block production
- State transitions
- Fork choice
- P2P gossip
- Mempool
- RPC API
