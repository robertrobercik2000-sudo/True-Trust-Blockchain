# 📋 PROPOZYCJA: Integracja Blockchain Node z Wallet CLI

## 🎯 Cel
Dodać **profesjonalną implementację blockchain node** łączącą:
- ✅ Istniejący PQ Wallet (Falcon512 + Kyber768) z `main.rs`
- ✅ Istniejący PoT consensus (`pot.rs`, `pot_node.rs`) - 765 linii
- ✅ Istniejący PoZS zkSNARK (`pozs.rs`, `pozs_groth16.rs`)

## 📁 Struktura - CO ZOSTANIE DODANE

### 1. Nowy moduł: `src/node_integration.rs` (~800 linii)

**Zawartość:**
```rust
//! Professional blockchain node integrating PoT + PoZS + PQ Wallet

pub struct BlockchainNode {
    // Wallet integration
    wallet: PqWalletKeys,
    
    // PoT consensus
    pot_node: PotNode,
    registry: Registry,
    trust_state: TrustState,
    
    // Network (simplified - Tokio TCP)
    peer_manager: PeerManager,
    
    // Storage (Sled DB)
    storage: BlockStorage,
    
    // Optional ZK prover
    #[cfg(feature = "zk-proofs")]
    zk_prover: Option<ZkProver>,
}

pub struct PqWalletKeys {
    // Re-use existing wallet structures from main.rs
    falcon_pk: Vec<u8>,
    falcon_sk: Zeroizing<Vec<u8>>,
    kyber_pk: Vec<u8>,
    kyber_sk: Zeroizing<Vec<u8>>,
}

impl BlockchainNode {
    pub fn new(config: NodeConfig, wallet_keys: PqWalletKeys) -> Result<Self>;
    pub async fn start(&self) -> Result<()>;
    pub async fn propose_block(&self, slot: u64) -> Result<Block>;
    pub async fn verify_block(&self, block: &Block) -> Result<bool>;
}
```

**API Design:**
- ✅ Używa istniejących typów z `pot.rs` i `pot_node.rs`
- ✅ Nie modyfikuje żadnego istniejącego kodu
- ✅ Async/await z Tokio
- ✅ Integracja z wallet przez bridge functions

### 2. Rozszerzenie CLI w `src/main.rs` (opcjonalne, ~150 linii dodane)

**Nowe subkomendy:**
```rust
#[derive(Subcommand, Debug)]
enum Cmd {
    // ... existing commands (wallet-init, wallet-addr, etc.) ...
    
    // ===== NEW: Node commands =====
    NodeInit {
        #[arg(long)] wallet_file: PathBuf,
        #[arg(long)] data_dir: PathBuf,
        #[arg(long)] stake: u64,
    },
    NodeRun {
        #[arg(long)] wallet_file: PathBuf,
        #[arg(long)] data_dir: PathBuf,
        #[arg(long)] listen: String,
        #[arg(long)] bootstrap: Vec<String>,
    },
    NodeStatus {
        #[arg(long)] data_dir: PathBuf,
    },
}
```

**Implementacja w main():**
```rust
Cmd::NodeRun { wallet_file, data_dir, listen, bootstrap } => {
    // 1. Unlock wallet (reuse existing code)
    let wallet_keys = unlock_wallet_to_node_keys(&wallet_file)?;
    
    // 2. Create node (NEW module)
    let node = BlockchainNode::new(
        NodeConfig {
            data_dir,
            listen_addr: listen,
            bootstrap_peers: bootstrap,
            ..Default::default()
        },
        wallet_keys,
    )?;
    
    // 3. Start node runtime
    node.start().await?;
}
```

### 3. Dependency Changes w `Cargo.toml` (5 linii dodane)

```toml
# Existing dependencies remain unchanged...

# NEW: Async runtime + networking (optional)
tokio = { version = "1.41", features = ["full"], optional = true }
sled = { version = "0.34", optional = true }  # Blockchain storage
tracing = { version = "0.1", optional = true }
tracing-subscriber = { version = "0.3", features = ["env-filter"], optional = true }

[features]
default = []
zk-proofs = [...]  # Existing
# NEW: Blockchain node feature
node = ["tokio", "sled", "tracing", "tracing-subscriber"]
```

### 4. Export w `src/lib.rs` (1 linia dodana)

```rust
// Existing modules...
pub mod crypto_kmac_consensus;
pub mod pot;
pub mod pot_node;
pub mod pozs;
pub mod snapshot;

// NEW: Node integration module (conditional)
#[cfg(feature = "node")]
pub mod node_integration;

// Existing exports...
pub use pot::{...};
pub use pot_node::{...};

// NEW: Node exports (conditional)
#[cfg(feature = "node")]
pub use node_integration::{BlockchainNode, NodeConfig, PqWalletKeys};
```

---

## 🔧 Szczegóły implementacji `node_integration.rs`

### A. Struktura danych

```rust
/// Blockchain configuration
#[derive(Clone, Debug)]
pub struct NodeConfig {
    pub data_dir: PathBuf,
    pub listen_addr: String,
    pub bootstrap_peers: Vec<String>,
    pub slot_duration_secs: u64,
    pub epoch_length: u64,
}

/// Block header with PoT witness
#[derive(Serialize, Deserialize, Clone)]
pub struct BlockHeader {
    pub slot: u64,
    pub epoch: u64,
    pub parent_hash: [u8; 32],
    pub state_root: [u8; 32],
    pub leader: NodeId,
    pub leader_witness: LeaderWitness,  // from pot.rs
    pub signature: Vec<u8>,             // Falcon512
    pub randao_reveal: [u8; 32],
    
    #[cfg(feature = "zk-proofs")]
    pub zk_proof: Option<Vec<u8>>,      // Groth16
}

/// Transaction
#[derive(Serialize, Deserialize, Clone)]
pub struct Transaction {
    pub from: NodeId,
    pub to: NodeId,
    pub amount: u64,
    pub nonce: u64,
    pub signature: Vec<u8>,
}

/// Block
#[derive(Serialize, Deserialize, Clone)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}
```

### B. Wallet Bridge Functions

```rust
/// Convert wallet file to node keys (bridge to main.rs wallet logic)
pub fn load_wallet_for_node(
    wallet_file: &Path,
    password: &str,
) -> Result<PqWalletKeys> {
    // Reuse WalletV5 decryption from main.rs
    // Extract Falcon + Kyber keys
    // Return PqWalletKeys struct
}

impl PqWalletKeys {
    /// Derive NodeId from Falcon public key
    pub fn node_id(&self) -> NodeId {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(b"NODE_ID.v1");
        h.update(&self.falcon_pk);
        let mut id = [0u8; 32];
        id.copy_from_slice(&h.finalize()[..32]);
        id
    }
    
    /// Sign with Falcon512
    pub fn sign_block(&self, block: &BlockHeader) -> Result<Vec<u8>> {
        use pqcrypto_falcon::falcon512;
        use pqcrypto_traits::sign::{SecretKey, DetachedSignature};
        
        let sk = falcon512::SecretKey::from_bytes(&self.falcon_sk)?;
        let msg = bincode::serialize(block)?;
        let sig = falcon512::detached_sign(&msg, &sk);
        Ok(sig.as_bytes().to_vec())
    }
}
```

### C. Network Layer (Simplified TCP)

```rust
/// Simple TCP peer manager (no libp2p to avoid version conflicts)
pub struct PeerManager {
    peers: Arc<RwLock<HashMap<SocketAddr, PeerConnection>>>,
    message_tx: mpsc::UnboundedSender<NetworkMessage>,
}

#[derive(Serialize, Deserialize)]
pub enum NetworkMessage {
    NewBlock(Block),
    BlockRequest(u64),
    BlockResponse(Option<Block>),
    RandaoCommit(Proposal),  // from pot.rs
    RandaoReveal(Proposal),
}

impl PeerManager {
    pub async fn listen(&self, addr: SocketAddr) -> Result<()>;
    pub async fn connect(&self, addr: SocketAddr) -> Result<()>;
    pub async fn broadcast(&self, msg: NetworkMessage) -> Result<()>;
    pub async fn recv(&mut self) -> Option<NetworkMessage>;
}
```

### D. Storage Layer (Sled DB)

```rust
pub struct BlockStorage {
    db: sled::Db,
}

impl BlockStorage {
    pub fn open(path: PathBuf) -> Result<Self>;
    pub fn store_block(&self, block: &Block) -> Result<()>;
    pub fn get_block(&self, slot: u64) -> Result<Option<Block>>;
    pub fn latest_block(&self) -> Result<Option<Block>>;
    pub fn store_snapshot(&self, epoch: u64, snapshot: &EpochSnapshot) -> Result<()>;
}
```

### E. Main Node Loop

```rust
impl BlockchainNode {
    pub async fn start(&self) -> Result<()> {
        // 1. Start network listener
        let addr: SocketAddr = self.config.listen_addr.parse()?;
        self.peer_manager.listen(addr).await?;
        
        // 2. Connect to bootstrap peers
        for peer in &self.config.bootstrap_peers {
            let addr: SocketAddr = peer.parse()?;
            self.peer_manager.connect(addr).await.ok();
        }
        
        // 3. Main event loop
        loop {
            tokio::select! {
                // Handle incoming network messages
                Some(msg) = self.peer_manager.recv() => {
                    self.handle_message(msg).await?;
                }
                
                // Handle slot timer
                _ = self.slot_timer.tick() => {
                    let slot = self.current_slot();
                    if self.is_leader(slot).await? {
                        let block = self.propose_block(slot).await?;
                        self.peer_manager.broadcast(
                            NetworkMessage::NewBlock(block)
                        ).await?;
                    }
                }
                
                // Handle Ctrl+C
                _ = tokio::signal::ctrl_c() => {
                    info!("Shutting down...");
                    break;
                }
            }
        }
        
        Ok(())
    }
    
    async fn handle_message(&self, msg: NetworkMessage) -> Result<()> {
        match msg {
            NetworkMessage::NewBlock(block) => {
                if self.verify_block(&block).await? {
                    self.storage.store_block(&block)?;
                    // Update trust via pot_node
                    self.update_trust(&block.header.leader, true).await?;
                }
            }
            NetworkMessage::RandaoCommit(proposal) => {
                self.pot_node.handle_randao_commit(proposal).await?;
            }
            // ... other messages
        }
        Ok(())
    }
}
```

---

## 📊 File Changes Summary

| File | Type | Lines | Description |
|------|------|-------|-------------|
| `src/node_integration.rs` | **NEW** | ~800 | Node implementation |
| `src/lib.rs` | Modified | +2 | Add `pub mod node_integration` |
| `Cargo.toml` | Modified | +5 | Add tokio, sled, tracing (optional) |
| `src/main.rs` | Modified | +150 | Add node subcommands (optional) |
| **TOTAL** | | **~960** | **New code** |

**Existing files NOT touched:**
- ✅ `pot.rs` (765 lines) - unchanged
- ✅ `pot_node.rs` (481 lines) - unchanged
- ✅ `pozs.rs` (460 lines) - unchanged
- ✅ `pozs_groth16.rs` (417 lines) - unchanged
- ✅ `crypto_kmac_consensus.rs` - unchanged
- ✅ `snapshot.rs` - unchanged
- ✅ Wallet logic in `main.rs` (first 1000 lines) - unchanged

---

## 🚀 Usage Examples

### Example 1: Initialize node with existing wallet

```bash
# Create wallet (existing command)
./tt_priv_cli wallet-init --file my_wallet.enc

# Initialize node with this wallet
./tt_priv_cli node-init \
  --wallet-file my_wallet.enc \
  --data-dir ./blockchain_data \
  --stake 1000000
```

### Example 2: Run validator node

```bash
./tt_priv_cli node-run \
  --wallet-file my_wallet.enc \
  --data-dir ./blockchain_data \
  --listen 0.0.0.0:8000 \
  --bootstrap 192.168.1.100:8000
  
# Output:
# 🔐 Unlocking wallet...
# ✓ Wallet unlocked (Falcon512 + Kyber768)
# 📊 Node ID: a3f8e9d2...
# 🚀 Starting PoT consensus...
# 🌐 Listening on 0.0.0.0:8000
# ✓ Connected to 192.168.1.100:8000
# [INFO] Epoch 0, slot 0 - waiting for leader selection
# [INFO] Registered RANDAO commitment
```

### Example 3: Programmatic usage (library)

```rust
use tt_priv_cli::node_integration::{BlockchainNode, NodeConfig, load_wallet_for_node};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load wallet
    let wallet_keys = load_wallet_for_node(
        Path::new("my_wallet.enc"),
        "my_password",
    )?;
    
    // Create node
    let node = BlockchainNode::new(
        NodeConfig {
            data_dir: PathBuf::from("./data"),
            listen_addr: "127.0.0.1:8000".into(),
            bootstrap_peers: vec![],
            slot_duration_secs: 6,
            epoch_length: 32,
        },
        wallet_keys,
    )?;
    
    // Start node
    node.start().await?;
    
    Ok(())
}
```

---

## ✅ Benefits of This Design

### 1. **Zero Breaking Changes**
- Existing wallet CLI works exactly as before
- Library users not affected
- Opt-in via `--features node`

### 2. **Professional Architecture**
- Clean separation: wallet logic vs node logic
- Async/await for scalability
- Persistent storage (Sled)
- Network gossip protocol

### 3. **Reuses Existing Code**
- `pot.rs` - leader selection, trust, RANDAO
- `pot_node.rs` - epoch management
- `pozs_groth16.rs` - ZK proofs (optional)
- Wallet encryption/signing from `main.rs`

### 4. **Production-Ready Features**
- Atomic file operations (inherited from wallet code)
- Error handling with `anyhow`
- Logging with `tracing`
- Graceful shutdown (Ctrl+C)
- Peer discovery and reconnection

---

## 🔒 Security Considerations

### 1. Wallet Key Storage
```rust
// Keys stay in memory only (Zeroizing)
pub struct PqWalletKeys {
    falcon_sk: Zeroizing<Vec<u8>>,  // Cleared on drop
    kyber_sk: Zeroizing<Vec<u8>>,
}
```

### 2. Network Security
- TODO: Add Noise protocol encryption (future)
- TODO: Peer authentication with PQ keys
- Current: Plaintext TCP (testnet only)

### 3. Block Verification
```rust
async fn verify_block(&self, block: &Block) -> Result<bool> {
    // 1. Verify Falcon512 signature
    // 2. Verify PoT leader witness
    // 3. Verify ZK proof (if feature enabled)
    // 4. Verify parent hash chain
    // 5. Verify RANDAO reveal
}
```

---

## 📝 Testing Plan

### Unit Tests (in `node_integration.rs`)
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_wallet_to_node_keys() { ... }
    
    #[test]
    fn test_block_signing() { ... }
    
    #[tokio::test]
    async fn test_node_creation() { ... }
}
```

### Integration Test
```bash
# Terminal 1
./tt_priv_cli node-run --wallet-file wallet1.enc --listen 127.0.0.1:8001

# Terminal 2  
./tt_priv_cli node-run --wallet-file wallet2.enc --listen 127.0.0.1:8002 --bootstrap 127.0.0.1:8001

# Terminal 3
./tt_priv_cli node-run --wallet-file wallet3.enc --listen 127.0.0.1:8003 --bootstrap 127.0.0.1:8001

# Expected: All 3 nodes sync, participate in RANDAO, propose blocks
```

---

## 🎯 Decision Points (Your Input Needed)

### Option A: Minimal Integration (Recommended)
- ✅ Add `node_integration.rs` module only
- ✅ NO changes to `main.rs` CLI
- ✅ Users import as library: `use tt_priv_cli::node_integration`
- Lines: ~800 new

### Option B: Full CLI Integration
- ✅ Add `node_integration.rs` module
- ✅ Add node subcommands to `main.rs`
- ✅ Unified binary for wallet + node
- Lines: ~950 new

### Option C: Separate Binary
- ✅ Add `src/bin/node.rs` (separate executable)
- ✅ Add `node_integration.rs` as library
- ✅ Zero changes to `main.rs`
- Produces 2 binaries: `tt_priv_cli` + `tt_node`
- Lines: ~900 new

---

## 💬 Questions for You

1. **Which option do you prefer?** (A, B, or C)

2. **CLI commands naming?**
   - Option 1: `node-init`, `node-run`, `node-status`
   - Option 2: `blockchain-init`, `blockchain-run`, `blockchain-status`
   - Option 3: `validator-init`, `validator-run`, `validator-status`

3. **Network protocol?**
   - Simple TCP gossip (fast to implement, testnet only)
   - Or wait for full Noise protocol? (production-ready, takes longer)

4. **Storage location?**
   - `~/.tt_blockchain/` (separate from wallet)
   - Or `~/.tt_wallet/blockchain/` (together with wallet)?

5. **ZK proof generation?**
   - On-demand only (command: `node-prove-eligibility`)
   - Or automatic for every block proposal?

---

## 📅 Implementation Timeline

**If approved:**
1. Create `node_integration.rs` skeleton (30 min)
2. Implement wallet bridge functions (30 min)
3. Implement network layer (2 hours)
4. Implement storage layer (1 hour)
5. Implement main node loop (2 hours)
6. Add CLI commands (if Option B/C) (1 hour)
7. Testing + documentation (2 hours)

**Total: ~9 hours** (can split into phases)

---

## 🤝 Your Approval Needed

**Please review and respond:**
- ✅ Approve as-is → I implement immediately
- 🔧 Request changes → I revise proposal
- ❌ Different approach → We discuss alternatives

**Once approved, I will:**
1. Create all new files
2. Show you the final diff before applying
3. Ensure all existing tests pass
4. Provide usage examples
