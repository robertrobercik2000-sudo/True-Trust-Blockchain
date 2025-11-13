//! Blockchain node implementation integrating PoT + PoZS + PQ Wallet

use crate::consensus::*;
use crate::wallet::PqWallet;
use crate::storage::BlockchainStorage;
use crate::network::{PeerManager, GossipLayer, NetworkMessage};
use anyhow::{Result, Context};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};

#[cfg(feature = "zk-proofs")]
use crate::zk::{setup_keys};
#[cfg(feature = "zk-proofs")]
use crate::zk::{EligibilityCircuit, EligibilityPublicInputs, EligibilityWitness};
#[cfg(feature = "zk-proofs")]
use crate::zk::ZkVerifyingKey;

/// Node configuration
#[derive(Debug, Clone)]
pub struct NodeConfig {
    pub data_dir: PathBuf,
    pub listen_addr: String,
    pub bootstrap_peers: Vec<String>,
    pub pot_params: PotParams,
    pub trust_params: TrustParams,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./data"),
            listen_addr: "127.0.0.1:8000".into(),
            bootstrap_peers: vec![],
            pot_params: PotParams::default(),
            trust_params: TrustParams::default(),
        }
    }
}

/// Main blockchain node
pub struct BlockchainNode {
    pub config: NodeConfig,
    pub wallet: Arc<RwLock<PqWallet>>,
    pub storage: Arc<BlockchainStorage>,
    pub peer_manager: Arc<RwLock<PeerManager>>,
    pub gossip: Arc<GossipLayer>,
    
    // Consensus state
    pub registry: Arc<RwLock<Registry>>,
    pub trust_state: Arc<RwLock<TrustState>>,
    pub randao: Arc<RwLock<RandaoBeacon>>,
    pub pot: Arc<pot::PotConsensus>,
    
    // ZK verifying key (if enabled)
    #[cfg(feature = "zk-proofs")]
    pub zk_vk: Option<Arc<ZkVerifyingKey>>,
}

impl BlockchainNode {
    /// Create new node
    pub fn new(config: NodeConfig, wallet: PqWallet) -> Result<Self> {
        // Initialize storage
        std::fs::create_dir_all(&config.data_dir)?;
        let storage = Arc::new(BlockchainStorage::open(config.data_dir.join("blockchain.db"))?);

        // Initialize networking
        let peer_manager = Arc::new(RwLock::new(PeerManager::new()));
        let gossip = Arc::new(GossipLayer::new(peer_manager.clone()));

        // Initialize consensus
        let registry = Arc::new(RwLock::new(Registry::new()));
        let trust_state = Arc::new(RwLock::new(TrustState::new()));
        let randao = Arc::new(RwLock::new(RandaoBeacon::new(0)));
        let pot = Arc::new(pot::PotConsensus::new(
            config.pot_params.clone(),
            config.trust_params.clone(),
        ));

        Ok(Self {
            config,
            wallet: Arc::new(RwLock::new(wallet)),
            storage,
            peer_manager,
            gossip,
            registry,
            trust_state,
            randao,
            pot,
            #[cfg(feature = "zk-proofs")]
            zk_vk: None,
        })
    }

    /// Initialize ZK proving system (if enabled)
    #[cfg(feature = "zk-proofs")]
    pub fn init_zk(&mut self) -> Result<()> {
        info!("Initializing Groth16 zkSNARK system...");
        
        // Setup keys (in production, load from trusted setup!)
        let dummy_circuit = EligibilityCircuit {
            public_inputs: Some(EligibilityPublicInputs {
                weights_root: [0u8; 32],
                beacon_value: [0u8; 32],
                threshold_q: 0,
                sum_weights_q: 0,
            }),
            witness: Some(EligibilityWitness {
                who: [0u8; 32],
                slot: 0,
                stake_q: 0,
                trust_q: 0,
                merkle_siblings: vec![],
                leaf_index: 0,
            }),
        };

        let (_pk, vk) = setup_keys(dummy_circuit)
            .context("Failed to setup ZK keys")?;
        
        self.zk_vk = Some(Arc::new(vk));
        info!("ZK system initialized successfully");
        
        Ok(())
    }

    /// Start node
    pub async fn start(&self) -> Result<()> {
        info!("Starting blockchain node...");
        info!("Listen address: {}", self.config.listen_addr);

        // Start network listener
        let addr: std::net::SocketAddr = self.config.listen_addr.parse()
            .context("Invalid listen address")?;
        
        let pm = self.peer_manager.read().await;
        pm.listen(addr).await?;
        drop(pm);

        // Connect to bootstrap peers
        for peer_addr in &self.config.bootstrap_peers {
            if let Ok(addr) = peer_addr.parse() {
                let pm = self.peer_manager.read().await;
                if let Err(e) = pm.connect(addr).await {
                    warn!("Failed to connect to {}: {}", peer_addr, e);
                }
            }
        }

        // Start message processing loop
        self.run_message_loop().await?;

        Ok(())
    }

    /// Message processing loop
    async fn run_message_loop(&self) -> Result<()> {
        let mut pm = self.peer_manager.write().await;
        
        loop {
            if let Some((peer_addr, msg)) = pm.recv().await {
                match msg {
                    NetworkMessage::NewBlock(block) => {
                        info!("Received block at slot {} from {}", block.header.slot, peer_addr);
                        if let Err(e) = self.process_block(block).await {
                            error!("Failed to process block: {}", e);
                        }
                    }
                    NetworkMessage::BlockRequest(slot) => {
                        info!("Block request for slot {} from {}", slot, peer_addr);
                        // TODO: Send block response
                    }
                    NetworkMessage::RandaoCommit(proposal) => {
                        info!("RANDAO commit from validator {:?}", proposal.who);
                        let mut randao = self.randao.write().await;
                        if let Err(e) = randao.commit(&proposal.who, proposal.commitment) {
                            warn!("Invalid RANDAO commit: {}", e);
                        }
                    }
                    NetworkMessage::RandaoReveal(proposal) => {
                        info!("RANDAO reveal from validator {:?}", proposal.who);
                        if let Some(secret) = proposal.reveal {
                            let mut randao = self.randao.write().await;
                            if let Err(e) = randao.reveal(&proposal.who, secret) {
                                warn!("Invalid RANDAO reveal: {}", e);
                            }
                        }
                    }
                    NetworkMessage::Ping => {
                        // TODO: Send pong
                    }
                    _ => {}
                }
            }
        }
    }

    /// Process incoming block
    async fn process_block(&self, block: Block) -> Result<()> {
        let slot = block.header.slot;
        
        // 1. Verify block signature
        // TODO: Implement signature verification

        // 2. Verify leader eligibility (with optional ZK proof)
        #[cfg(feature = "zk-proofs")]
        if let Some(zk_proof_bytes) = &block.header.zk_proof {
            info!("Verifying ZK proof for block at slot {}", slot);
            // TODO: Deserialize and verify ZK proof
        }

        // 3. Store block
        self.storage.store_block(&block)?;
        
        // 4. Update trust state
        let mut trust = self.trust_state.write().await;
        self.pot.update_trust(&mut trust, &block.header.leader, true);
        drop(trust);

        // 5. Gossip to peers
        self.gossip.gossip_block(block).await?;

        info!("Block at slot {} processed successfully", slot);
        Ok(())
    }

    /// Get node ID
    pub async fn node_id(&self) -> Result<[u8; 32]> {
        let wallet = self.wallet.read().await;
        wallet.node_id()
    }

    /// Get current slot
    pub fn current_slot(&self) -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now / self.config.pot_params.slot_duration
    }

    /// Check if this node is leader for given slot
    pub async fn is_leader(&self, slot: u64) -> Result<bool> {
        let node_id = self.node_id().await?;
        let epoch = slot / self.config.pot_params.epoch_length;

        // Get snapshot
        let snapshot = self.storage.get_snapshot(epoch)?
            .ok_or_else(|| anyhow::anyhow!("Snapshot not found for epoch {}", epoch))?;

        // Get beacon value
        let randao = self.randao.read().await;
        let beacon = randao.value(epoch, slot);
        drop(randao);

        // Get trust
        let trust = self.trust_state.read().await;
        let trust_q = trust.get_trust(&node_id);
        drop(trust);

        // Get stake
        let registry = self.registry.read().await;
        let validator = registry.get(&node_id)
            .ok_or_else(|| anyhow::anyhow!("Not a registered validator"))?;
        let stake_q = validator.stake;
        drop(registry);

        // Compute eligibility
        let elig = self.pot.elig_hash(&beacon, slot, &node_id);
        let weight = snapshot.get_weight(&node_id)
            .ok_or_else(|| anyhow::anyhow!("Weight not found"))?;
        
        let threshold = self.pot.compute_threshold_q(weight, snapshot.total_weight_q)
            .ok_or_else(|| anyhow::anyhow!("Threshold overflow"))?;
        
        let bound = snapshot::qmul(threshold, u64::MAX as snapshot::Q);
        
        Ok(elig < bound)
    }
}
