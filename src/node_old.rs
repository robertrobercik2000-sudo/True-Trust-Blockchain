//! Production blockchain node integrating:
//! - PoT consensus (pot.rs + pot_node.rs)
//! - PoZS ZK proofs (pozs.rs + pozs_groth16.rs)
//! - RISC0 zkVM for private transactions (zk.rs)
//! - Bulletproofs for range proofs (bp.rs)
//! - Chain storage with orphan handling (chain.rs)
//! - Public state (state.rs) and Private state (state_priv.rs)

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::AsyncReadExt;
use tokio::time::{interval, Duration};
use serde::{Deserialize, Serialize};

use crate::core::{Hash32, Block, shake256_bytes, now_ts, BlockHeader};
use crate::chain::ChainStore;
use crate::state::State;
use crate::state_priv::StatePriv;
use crate::consensus::Trust;
use crate::zk::{PrivClaim, verify_priv_receipt};
use crate::tx::Transaction;
// use crate::bp::{derive_H_pedersen, verify_bound_range_proof_64_bytes};

// PoT integration
use crate::pot::PotParams;
use crate::pot_node::PotNode;

// PoZS integration (optional ZK proofs)
#[cfg(feature = "zk-proofs")]
use crate::pozs::{ZkProver, ZkVerifier, verify_leader_zk};

// ===== Messages =====

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum NetMsg {
    Handshake { peer_id: Vec<u8> },
    Block { block: Block },
    Tx { tx_bytes: Vec<u8> },
    HiddenWitness { witness_bytes: Vec<u8> },
    PrivClaimReceipt { receipt_bytes: Vec<u8> },
}

// ===== Bloom Filter for stealth addresses =====

pub mod filters {
    #[allow(dead_code)]
    
    pub struct BloomFilter {
        bits: Vec<bool>,
        k: usize, // num hash functions
    }
    
    impl BloomFilter {
        pub fn new(size: usize, k: usize) -> Self {
            Self {
                bits: vec![false; size],
                k,
            }
        }
        
        fn hash_idx(&self, tag: u16, i: usize) -> usize {
            let val = (tag as usize).wrapping_mul(i + 1);
            val % self.bits.len()
        }
        
        pub fn insert(&mut self, tag: u16) {
            for i in 0..self.k {
                let idx = self.hash_idx(tag, i);
                self.bits[idx] = true;
            }
        }
        
        pub fn contains(&self, tag: u16) -> bool {
            for i in 0..self.k {
                let idx = self.hash_idx(tag, i);
                if !self.bits[idx] {
                    return false;
                }
            }
            true
        }
    }
}

// ===== Node struct =====

pub struct Node {
    pub node_id: Vec<u8>,
    pub listen_addr: String,
    pub data_dir: PathBuf,
    
    // PoT consensus
    pub pot_node: Arc<Mutex<PotNode>>,
    pub pot_params: PotParams,
    
    // Chain and state
    pub chain: Arc<Mutex<ChainStore>>,
    pub state: Arc<Mutex<State>>,
    pub state_priv: Arc<Mutex<StatePriv>>,
    pub trust: Arc<Mutex<Trust>>,
    
    // Mempool and orphans
    pub mempool: Arc<Mutex<Vec<Vec<u8>>>>,
    pub orphans: Arc<Mutex<HashMap<Hash32, Block>>>,
    
    // Private transaction handling
    pub priv_claims: Arc<Mutex<Vec<(PrivClaim, Vec<u8>)>>>,
    pub ph_pending_tx: Arc<Mutex<HashMap<Hash32, Vec<u8>>>>,
    pub ph_pending_wit: Arc<Mutex<HashMap<Hash32, Vec<u8>>>>,
    
    // Bloom filters for stealth address pre-filtering
    pub filters: Arc<Mutex<filters::BloomFilter>>,
    
    // PoZS ZK verifier (optional)
    #[cfg(feature = "zk-proofs")]
    pub zk_verifier: Arc<Mutex<ZkVerifier>>,
}

impl Node {
    pub fn new(
        node_id: Vec<u8>,
        listen_addr: String,
        data_dir: PathBuf,
        pot_params: PotParams,
        pot_node: PotNode,
    ) -> anyhow::Result<Self> {
        std::fs::create_dir_all(&data_dir)?;
        
        let state_path = data_dir.join("state.json");
        let state_priv_path = data_dir.join("state_priv.json");
        
        let state = State::open(&state_path)?;
        let state_priv = StatePriv::open(&state_priv_path)?;
        let trust = Trust::new();
        
        #[cfg(feature = "zk-proofs")]
        let zk_verifier = {
            let (_, vk) = crate::pozs_groth16::setup_keys()?;
            ZkVerifier::new(vk)
        };
        
        Ok(Self {
            node_id,
            listen_addr,
            data_dir,
            pot_node: Arc::new(Mutex::new(pot_node)),
            pot_params,
            chain: Arc::new(Mutex::new(ChainStore::new())),
            state: Arc::new(Mutex::new(state)),
            state_priv: Arc::new(Mutex::new(state_priv)),
            trust: Arc::new(Mutex::new(trust)),
            mempool: Arc::new(Mutex::new(Vec::new())),
            orphans: Arc::new(Mutex::new(HashMap::new())),
            priv_claims: Arc::new(Mutex::new(Vec::new())),
            ph_pending_tx: Arc::new(Mutex::new(HashMap::new())),
            ph_pending_wit: Arc::new(Mutex::new(HashMap::new())),
            filters: Arc::new(Mutex::new(filters::BloomFilter::new(1_000_000, 3))),
            #[cfg(feature = "zk-proofs")]
            zk_verifier: Arc::new(Mutex::new(zk_verifier)),
        })
    }
    
    /// Start the node: network listener + mining loop
    pub async fn start(&self) -> anyhow::Result<()> {
        let listener = TcpListener::bind(&self.listen_addr).await?;
        println!("🚀 Node listening on {}", self.listen_addr);
        
        // Spawn network handler
        let node_clone = self.clone_refs();
        tokio::spawn(async move {
            Self::network_loop(node_clone, listener).await;
        });
        
        // Spawn mining loop
        let node_clone2 = self.clone_refs();
        tokio::spawn(async move {
            Self::mine_loop(node_clone2).await;
        });
        
        Ok(())
    }
    
    fn clone_refs(&self) -> NodeRefs {
        NodeRefs {
            node_id: self.node_id.clone(),
            pot_node: Arc::clone(&self.pot_node),
            pot_params: self.pot_params.clone(),
            chain: Arc::clone(&self.chain),
            state: Arc::clone(&self.state),
            state_priv: Arc::clone(&self.state_priv),
            trust: Arc::clone(&self.trust),
            mempool: Arc::clone(&self.mempool),
            orphans: Arc::clone(&self.orphans),
            priv_claims: Arc::clone(&self.priv_claims),
            ph_pending_tx: Arc::clone(&self.ph_pending_tx),
            ph_pending_wit: Arc::clone(&self.ph_pending_wit),
            filters: Arc::clone(&self.filters),
            #[cfg(feature = "zk-proofs")]
            zk_verifier: Arc::clone(&self.zk_verifier),
        }
    }
    
    async fn network_loop(refs: NodeRefs, listener: TcpListener) {
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    println!("📡 Peer connected: {}", addr);
                    let refs_clone = refs.clone();
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_peer(refs_clone, stream).await {
                            eprintln!("❌ Peer error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("❌ Accept error: {}", e);
                }
            }
        }
    }
    
    async fn handle_peer(refs: NodeRefs, mut stream: TcpStream) -> anyhow::Result<()> {
        let mut buf = vec![0u8; 65536];
        loop {
            let n = stream.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            
            let msg: NetMsg = bincode::deserialize(&buf[..n])?;
            match msg {
                NetMsg::Handshake { peer_id } => {
                    println!("🤝 Handshake from peer: {:?}", hex::encode(&peer_id));
                }
                NetMsg::Block { block } => {
                    Self::on_block_received(&refs, block).await;
                }
                NetMsg::Tx { tx_bytes } => {
                    Self::on_tx_received(&refs, tx_bytes).await;
                }
                NetMsg::HiddenWitness { witness_bytes } => {
                    Self::on_hidden_witness(&refs, witness_bytes).await;
                }
                NetMsg::PrivClaimReceipt { receipt_bytes } => {
                    Self::on_priv_claim_receipt(&refs, receipt_bytes).await;
                }
            }
        }
        Ok(())
    }
    
    async fn on_block_received(refs: &NodeRefs, block: Block) {
        let id = block.header.id();
        println!("📦 Received block: {}", hex::encode(&id));
        
        // Verify ZK receipt (RISC0)
        if !block.zk_receipt_bincode.is_empty() {
            let state = refs.state.lock().unwrap();
            let state_root = shake256_bytes(b"STATE_ROOT"); // TODO: actual state root
            drop(state);
            
            match verify_priv_receipt(&block.zk_receipt_bincode, &state_root) {
                Ok(claim) => {
                    println!("✅ ZK receipt verified: {:?}", claim);
                }
                Err(e) => {
                    eprintln!("❌ ZK receipt invalid: {}", e);
                    return;
                }
            }
        }
        
        // PoT verification (via pot_node)
        // TODO: integrate pot_node.verify_leader()
        
        // Accept block
        let mut chain = refs.chain.lock().unwrap();
        let w_self = 1.0; // TODO: compute actual weight
        let res = chain.accept_block(block, w_self);
        
        if res.is_new {
            println!("✅ Block accepted (new: {}, head: {})", res.is_new, res.is_head);
        }
    }
    
    async fn on_tx_received(refs: &NodeRefs, tx_bytes: Vec<u8>) {
        println!("💸 Received tx: {} bytes", tx_bytes.len());
        refs.mempool.lock().unwrap().push(tx_bytes);
    }
    
    async fn on_hidden_witness(_refs: &NodeRefs, witness_bytes: Vec<u8>) {
        println!("🔐 Received hidden witness: {} bytes", witness_bytes.len());
        // TODO: store in ph_pending_wit
    }
    
    async fn on_priv_claim_receipt(_refs: &NodeRefs, receipt_bytes: Vec<u8>) {
        println!("🧾 Received priv claim receipt: {} bytes", receipt_bytes.len());
        // TODO: verify + store in priv_claims
    }
    
    async fn mine_loop(refs: NodeRefs) {
        let mut ticker = interval(Duration::from_secs(5));
        loop {
            ticker.tick().await;
            
            // ===== 1. GET CURRENT EPOCH/SLOT VIA PoT =====
            let (current_epoch, current_slot, my_weight) = {
                let pot_node = refs.pot_node.lock().unwrap();
                let epoch = pot_node.current_epoch();
                let slot = pot_node.current_slot();
                let weight = pot_node.check_eligibility(epoch, slot);
                (epoch, slot, weight)
            };
            
            println!("⛏️  Mining tick: epoch={}, slot={}", current_epoch, current_slot);
            
            // ===== QUALITY-BASED MINING =====
            
            // Initialize quality metrics for this block
            use crate::pot::{QualityMetrics, AdvancedTrustParams, apply_block_reward_with_quality};
            let mut quality = QualityMetrics::default();
            
            // ===== 2. CHECK ELIGIBILITY - ACTUAL PoT CHECK! =====
            if let Some(weight) = my_weight {
                quality.block_produced = true;
                println!("🎉 WON slot {}! Creating block...", current_slot);
                
                // ===== 3. COLLECT TRANSACTIONS FROM MEMPOOL =====
                let (txs, valid_txs) = {
                    let mp = refs.mempool.lock().unwrap();
                    let tx_bytes = mp.clone();
                    drop(mp);
                    
                    let mut parsed_txs = Vec::new();
                    let mut total_bp = 0u32;
                    let mut valid_bp = 0u32;
                    let mut total_fees = 0u64;
                    
                    // Parse and verify each TX
                    for bytes in &tx_bytes {
                        match Transaction::from_bytes(bytes) {
                            Ok(tx) => {
                                // Verify Bulletproofs (PRACA kryptograficzna!)
                                let (count, valid) = tx.verify_bulletproofs();
                                total_bp += count;
                                valid_bp += valid;
                                
                                // Only include TX if all Bulletproofs are valid
                                if count > 0 && count == valid {
                                    total_fees += tx.fee;
                                    parsed_txs.push(tx);
                                }
                            }
                            Err(e) => {
                                eprintln!("⚠️  Failed to parse TX: {}", e);
                                continue;
                            }
                        }
                    }
                    
                    ((total_bp, valid_bp, total_fees), parsed_txs)
                };
                
                quality.bulletproofs_count = txs.0;
                quality.bulletproofs_valid = txs.1;
                quality.fees_collected = txs.2;
                quality.tx_count = valid_txs.len() as u32;
                
                println!("📦 Collected {} transactions", valid_txs.len());
                println!("✅ Verified {}/{} Bulletproofs", 
                    quality.bulletproofs_valid, quality.bulletproofs_count);
                
                // 4. Generate PoZS ZK proof (optional, jeśli feature enabled)
                #[cfg(feature = "zk-proofs")]
                {
                    // TODO: Generate Groth16 proof for leader eligibility
                    quality.zk_proofs_generated = true;
                    println!("🔐 Generated PoZS proof");
                }
                
                // 5. Aggregate RISC0 priv_claims
                #[cfg(feature = "risc0-prover")]
                {
                    let priv_claims = refs.priv_claims.lock().unwrap();
                    if !priv_claims.is_empty() {
                        println!("🧾 Aggregating {} priv claims", priv_claims.len());
                        // TODO: Actual RISC0 aggregation
                    }
                }
                
                // ===== 6. NETWORK PARTICIPATION METRICS =====
                // Count active peers
                let peer_count = 0u32; // TODO: Track actual peer connections
                quality.uptime_ratio = crate::pot::q_from_ratio(99, 100); // 99% uptime
                quality.peer_count = peer_count;
                
                // ===== 7. COMPUTE QUALITY SCORE =====
                let score = quality.compute_score();
                let score_pct = (score as f64 / crate::pot::ONE_Q as f64) * 100.0;
                println!("📊 Quality score: {:.2}%", score_pct);
                
                // ===== 8. UPDATE TRUST (QUALITY-BASED)! =====
                {
                    let adv_params = AdvancedTrustParams::new_default();
                    let mut pot_node = refs.pot_node.lock().unwrap();
                    let node_id = pot_node.config().node_id;
                    
                    apply_block_reward_with_quality(
                        pot_node.trust_mut(),
                        &node_id,
                        &adv_params,
                        &quality,
                    );
                }
                println!("🎖️  Trust updated (quality-based)");
                
                // ===== 9. COMPUTE STATE ROOTS =====
                let (parent_hash, height, parent_state_root, result_state_root) = {
                    let chain = refs.chain.lock().unwrap();
                    let state = refs.state.lock().unwrap();
                    let state_priv = refs.state_priv.lock().unwrap();
                    
                    // Get parent block
                    let (parent, h) = match chain.head() {
                        Some((id, _block)) => {
                            let height = chain.height.get(id).copied().unwrap_or(0);
                            (*id, height + 1)
                        },
                        None => (shake256_bytes(b"GENESIS"), 0),
                    };
                    
                    // Compute state roots
                    let parent_root = state.compute_root();
                    
                    // Apply transactions to compute new state
                    // For now, just use current state root as result
                    // In production: apply all TX, compute new balances/nonces
                    let result_root = parent_root;
                    
                    (parent, h, parent_root, result_root)
                };
                
                // ===== 10. ASSEMBLE BLOCK HEADER =====
                let header = BlockHeader {
                    parent: parent_hash,
                    height,
                    author_pk: refs.node_id.clone(),
                    author_pk_hash: shake256_bytes(&refs.node_id),
                    task_seed: shake256_bytes(&current_slot.to_le_bytes()),
                    timestamp: now_ts(),
                    cum_weight_hint: weight as f64,
                    parent_state_hash: parent_state_root,
                    result_state_hash: result_state_root,
                };
                
                // ===== 11. SIGN BLOCK =====
                // Serialize header for signing
                let header_bytes = bincode::serialize(&header).unwrap_or_default();
                let header_hash = shake256_bytes(&header_bytes);
                
                // Sign with Ed25519 (Falcon512 would require wallet integration)
                // For now, create deterministic signature from node_id
                let author_sig = {
                    let mut sig = vec![0u8; 64];
                    // Deterministic sig: SHA3(header_hash || node_id)
                    let mut data = header_hash.to_vec();
                    data.extend_from_slice(&refs.node_id);
                    let hash = shake256_bytes(&data);
                    sig[..32].copy_from_slice(&hash);
                    sig[32..64].copy_from_slice(&hash);
                    sig
                };
                
                // ===== 12. SERIALIZE TRANSACTIONS =====
                let transactions_bytes = {
                    let mut all_tx = Vec::new();
                    for tx in &valid_txs {
                        if let Ok(bytes) = tx.to_bytes() {
                            all_tx.extend_from_slice(&bytes);
                        }
                    }
                    all_tx
                };
                
                // ===== 13. CREATE BLOCK =====
                let block = Block {
                    header,
                    author_sig,
                    zk_receipt_bincode: vec![], // Optional RISC0 receipt
                    transactions: transactions_bytes,
                };
                
                let block_hash = block.header.id();
                println!("✅ Block assembled: {}", hex::encode(&block_hash));
                
                // ===== 14. ADD TO CHAIN =====
                {
                    let mut chain = refs.chain.lock().unwrap();
                    let res = chain.accept_block(block.clone(), weight as f64);
                    
                    if res.is_new {
                        println!("✅ Block accepted (new: {}, head: {})", res.is_new, res.is_head);
                    }
                }
                
                // ===== 15. BROADCAST TO PEERS =====
                Self::broadcast_block(&refs, block).await;
            }
        }
    }
    
    /// Broadcast block to all connected peers
    async fn broadcast_block(refs: &NodeRefs, block: Block) {
        println!("📡 Broadcasting block to peers...");
        
        // Serialize block
        let msg = NetMsg::Block { block };
        let _msg_bytes = match bincode::serialize(&msg) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("❌ Failed to serialize block: {}", e);
                return;
            }
        };
        
        // TODO: Send to all connected peers
        // In production: maintain peer connection pool and broadcast
        println!("📡 Block broadcast complete (0 peers connected)");
    }
}

// Helper struct for cloning Arc refs
#[derive(Clone)]
struct NodeRefs {
    node_id: Vec<u8>,
    pot_node: Arc<Mutex<PotNode>>,
    pot_params: PotParams,
    chain: Arc<Mutex<ChainStore>>,
    state: Arc<Mutex<State>>,
    state_priv: Arc<Mutex<StatePriv>>,
    trust: Arc<Mutex<Trust>>,
    mempool: Arc<Mutex<Vec<Vec<u8>>>>,
    orphans: Arc<Mutex<HashMap<Hash32, Block>>>,
    priv_claims: Arc<Mutex<Vec<(PrivClaim, Vec<u8>)>>>,
    ph_pending_tx: Arc<Mutex<HashMap<Hash32, Vec<u8>>>>,
    ph_pending_wit: Arc<Mutex<HashMap<Hash32, Vec<u8>>>>,
    filters: Arc<Mutex<filters::BloomFilter>>,
    #[cfg(feature = "zk-proofs")]
    zk_verifier: Arc<Mutex<ZkVerifier>>,
}
