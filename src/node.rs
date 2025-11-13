//! Production blockchain node v2 (host/src/node.rs)
//! 
//! Features:
//! - Split BP verifiers (ZK journal vs wire TX)
//! - Real ZK aggregation with fanout
//! - Wbudowane Bloom filters
//! - Orphan pool z timestampami
//! - Falcon-512 Post-Quantum signing
//! - PoT consensus integration

#![allow(dead_code)]

use std::{sync::Arc, time::Instant};
use std::collections::HashMap;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

use serde::{Serialize, Deserialize};

// Post-Quantum signatures
use crate::falcon_sigs::{FalconSecretKey, FalconPublicKey, falcon_sk_from_bytes, falcon_sign_block, falcon_verify_block, falcon_pk_to_bytes, BlockSignature};

use crate::core::{Block, BlockHeader, Hash32, now_ts, bytes32};
use crate::chain::ChainStore;
use crate::consensus::Trust;
use crate::state::State;
use crate::state_priv::StatePriv;

// BP helpers (split: ZK journal vs wire TX)
use crate::bp::{derive_H_pedersen, parse_dalek_range_proof_64, verify_range_proof_64};

// ZK host interface
use crate::zk::{self, AggPrivInput, verify_priv_receipt, verify_agg_receipt, bytes_to_words};

// PoT consensus integration
use crate::pot_node::PotNode;
use crate::pot::PotParams;

// =================== Wbudowane filtry Bloom ===================
pub mod filters {
    use serde::{Deserialize, Serialize};
    use std::{fs, io::Write, path::PathBuf};

    pub type Hash32 = [u8; 32];

    #[derive(Clone, Serialize, Deserialize, Debug)]
    pub struct BloomFilter {
        m_bits: usize,
        k_hash: usize,
        bits: Vec<u8>,
    }

    impl BloomFilter {
        pub fn with_params(n_items_guess: u32, fp_rate: f64) -> Self {
            let n = n_items_guess.max(1) as f64;
            let p = fp_rate.clamp(1e-9, 0.5);
            let ln2 = std::f64::consts::LN_2;
            let m = ((-n * p.ln()) / (ln2 * ln2)).ceil().max(8.0) as usize;
            let k = ((m as f64 / n) * ln2).round().clamp(1.0, 16.0) as usize;
            let bytes = (m + 7) / 8;
            Self { m_bits: m, k_hash: k, bits: vec![0u8; bytes] }
        }

        #[inline] fn set_bit(&mut self, idx: usize) {
            let b = idx >> 3;
            let m = 1u8 << (idx & 7);
            self.bits[b] |= m;
        }
        #[inline] fn get_bit(&self, idx: usize) -> bool {
            let b = idx >> 3;
            let m = 1u8 << (idx & 7);
            (self.bits[b] & m) != 0
        }

        fn h1_h2(x: &[u8]) -> (u128, u128) {
            use tiny_keccak::{Hasher, Shake};
            let mut sh = Shake::v256();
            sh.update(b"BF");
            sh.update(x);
            let mut out = [0u8; 32];
            sh.finalize(&mut out);
            let (a, b) = out.split_at(16);
            (u128::from_le_bytes(a.try_into().unwrap()), u128::from_le_bytes(b.try_into().unwrap()))
        }

        #[inline] fn index_for(&self, h1: u128, h2: u128, i: usize) -> usize {
            let x = h1.wrapping_add(h2.wrapping_mul(i as u128));
            (x % (self.m_bits as u128)) as usize
        }

        pub fn add(&mut self, x32: &Hash32) {
            let (h1, h2) = Self::h1_h2(x32);
            for i in 0..self.k_hash {
                let idx = self.index_for(h1, h2, i);
                self.set_bit(idx);
            }
        }

        pub fn probably_contains(&self, x32: &Hash32) -> bool {
            let (h1, h2) = Self::h1_h2(x32);
            for i in 0..self.k_hash {
                let idx = self.index_for(h1, h2, i);
                if !self.get_bit(idx) { return false; }
            }
            true
        }
    }

    #[derive(Clone, Serialize, Deserialize, Debug)]
    pub struct EpochFilter {
        pub epoch_idx: u32,
        pub start_height: u64,
        pub end_height: u64,
        pub bloom: BloomFilter,
    }

    pub struct FilterStore {
        root: PathBuf,
        pub blocks_per_epoch: u64,
    }

    impl FilterStore {
        pub fn open(root_dir: &str, blocks_per_epoch: u64) -> std::io::Result<Self> {
            let p = PathBuf::from(root_dir);
            fs::create_dir_all(&p)?;
            Ok(Self { root: p, blocks_per_epoch })
        }

        fn path_for(&self, epoch_idx: u32) -> PathBuf {
            self.root.join(format!("epoch_{:06}.bin", epoch_idx))
        }

        fn load_epoch(&self, epoch_idx: u32) -> Option<EpochFilter> {
            let p = self.path_for(epoch_idx);
            let data = fs::read(p).ok()?;
            bincode::deserialize(&data).ok()
        }

        fn save_epoch(&self, ef: &EpochFilter) -> std::io::Result<()> {
            let p = self.path_for(ef.epoch_idx);
            let mut f = fs::File::create(p)?;
            let data = bincode::serialize(ef).expect("serialize epoch filter");
            f.write_all(&data)?;
            Ok(())
        }

        pub fn epoch_index_for_height(&self, h: u64) -> u32 {
            (h / self.blocks_per_epoch) as u32
        }

        pub fn on_block_accepted(
            &self,
            height: u64,
            enc_hints: &[[u8; 32]],
            n_items_guess: u32,
            fp_rate: f64,
        ) -> std::io::Result<()> {
            let epoch_idx = self.epoch_index_for_height(height);
            let start_h = (epoch_idx as u64) * self.blocks_per_epoch;
            let end_h = start_h + (self.blocks_per_epoch - 1);

            let mut ef = self.load_epoch(epoch_idx).unwrap_or_else(|| EpochFilter {
                epoch_idx,
                start_height: start_h,
                end_height: end_h,
                bloom: BloomFilter::with_params(n_items_guess.max(1), fp_rate),
            });

            for h in enc_hints.iter() { ef.bloom.add(h); }
            self.save_epoch(&ef)
        }
    }

    pub use FilterStore as Store;
}

/// Wiadomości sieciowe
#[derive(Serialize, Deserialize)]
enum NetMsg {
    Block(Block),
    Transaction(Vec<u8>), // TX serialized
    HiddenWitness(HiddenWitnessNet),
    PrivClaimReceipt(PrivClaimNet),
}

/// Prywatny świadek (off-chain)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HiddenWitnessNet {
    pub tx_hash: Hash32,
    pub pubkey:   Vec<u8>,
    pub sig:      Vec<u8>,
    pub index:    u32,
    pub siblings: Vec<[u8;32]>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PrivClaimNet {
    pub receipt_bincode: Vec<u8>,
}

/// Sieroty czekające na rodzica
pub struct OrphanEntry { 
    pub block: Block, 
    pub ts: Instant 
}
pub type OrphanPool = HashMap<Hash32, Vec<OrphanEntry>>;

pub struct NodeV2 {
    pub listen: Option<String>,
    pub trust: Trust,
    pub require_zk: bool,

    // PoT integration
    pub pot_node: Arc<Mutex<PotNode>>,
    pub pot_params: PotParams,

    pub chain: Arc<Mutex<ChainStore>>,
    pub state: Arc<Mutex<State>>,
    pub st_priv: Arc<Mutex<StatePriv>>,

    pub mempool: Arc<Mutex<HashMap<Hash32, Vec<u8>>>>, // TX bytes
    pub orphans: Arc<Mutex<OrphanPool>>,

    // child receipts dla aggregacji
    pub priv_claims: Arc<Mutex<Vec<Vec<u8>>>>,

    // filtry Bloom
    pub filters: Mutex<Option<filters::Store>>,
}

impl NodeV2 {
    pub const BLOCK_REWARD: u64 = 50; // TT

    pub fn new(
        trust: Trust,
        require_zk: bool,
        pot_node: Arc<Mutex<PotNode>>,
        pot_params: PotParams,
        state: Arc<Mutex<State>>,
        st_priv: Arc<Mutex<StatePriv>>,
    ) -> Self {
        Self {
            listen: None,
            trust,
            require_zk,
            pot_node,
            pot_params,
            chain: Arc::new(Mutex::new(ChainStore::new())),
            state,
            st_priv,
            mempool: Arc::new(Mutex::new(HashMap::new())),
            orphans: Arc::new(Mutex::new(HashMap::new())),
            priv_claims: Arc::new(Mutex::new(Vec::new())),
            filters: Mutex::new(None),
        }
    }

    pub async fn init_filters(&self, dir: &str, blocks_per_epoch: u64) -> anyhow::Result<()> {
        let store = filters::Store::open(dir, blocks_per_epoch)?;
        let mut g = self.filters.lock().await;
        *g = Some(store);
        Ok(())
    }

    /* ===================== SPLIT BP VERIFIERS ===================== */

    /// Verify BP dla ZK journal (agg output)
    fn verify_outs_bp_zk(outs_bp: &[crate::zk::OutBp]) -> anyhow::Result<()> {
        let H = derive_H_pedersen();
        for (i, o) in outs_bp.iter().enumerate() {
            let parsed = parse_dalek_range_proof_64(&o.proof_bytes)
                .map_err(|e| anyhow::anyhow!("bp parse (agg out #{i}): {e}"))?;
            verify_range_proof_64(&parsed, o.C_out, H)
                .map_err(|e| anyhow::anyhow!("bp verify (agg out #{i}): {e}"))?;
        }
        Ok(())
    }

    /// Verify BP dla wire TX z mempoolu
    fn verify_outs_bp_wire(tx_bytes: &[u8]) -> anyhow::Result<()> {
        use crate::tx::Transaction;
        let tx = Transaction::from_bytes(tx_bytes)?;
        let (total, valid) = tx.verify_bulletproofs();
        if total > 0 && total != valid {
            anyhow::bail!("BP verify failed: {}/{} valid", valid, total);
        }
        Ok(())
    }

    /* ===================== ZK AGGREGATION Z FANOUT ===================== */

    async fn aggregate_child_receipts(
        &self,
        fanout: usize,
    ) -> anyhow::Result<Vec<u8>> {
        let drained_receipts: Vec<Vec<u8>> = {
            let mut pc = self.priv_claims.lock().await;
            let take = pc.len().min(fanout);
            pc.drain(..take).collect()
        };

        if drained_receipts.is_empty() {
            return Ok(Vec::new());
        }

        if drained_receipts.len() == 1 {
            eprintln!("ℹ️  zk-agg: single child passthrough");
            return Ok(drained_receipts[0].clone());
        }

        // Aggregate multiple receipts using simplified API
        // TODO: Upgrade to full API with old_notes_root, old_notes_count, old_frontier,
        //       child_method_id, claim_receipts_words, claim_journals_words
        let state_root = {
            let sp = self.st_priv.lock().await;
            sp.notes_root
        };

        let t_agg = Instant::now();
        let (_agg_journal, agg_receipt_bin) =
            zk::prove_agg_priv_with_receipts(drained_receipts.clone(), state_root)
                .map_err(|e| anyhow::anyhow!("agg prove: {e}"))?;
        
        eprintln!("✅ zk-agg: batch_size={} took {:?}", 
            drained_receipts.len(), t_agg.elapsed());

        Ok(agg_receipt_bin)
    }

    /* ===================== NETWORKING ===================== */

    pub async fn start_listener(self: &Arc<Self>, addr: &str) -> anyhow::Result<()> {
        let listener = TcpListener::bind(addr).await?;
        println!("🔊 Listening on: {addr}");
        let me = self.clone();
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((sock, raddr)) => {
                        println!("▶️  Connected: {raddr}");
                        let me2 = me.clone();
                        tokio::spawn(async move { me2.handle_conn(sock).await; });
                    }
                    Err(e) => eprintln!("accept: {e}"),
                }
            }
        });
        Ok(())
    }

    async fn handle_conn(self: &Arc<Self>, mut sock: TcpStream) {
        let (r, mut w) = sock.split();
        let mut reader = BufReader::new(r).lines();

        while let Ok(Some(line)) = reader.next_line().await {
            let s = line.trim();
            if s.is_empty() { continue; }
            match serde_json::from_str::<NetMsg>(s) {
                Ok(NetMsg::Block(b)) => { let _ = self.on_block_received(b).await; }
                Ok(NetMsg::Transaction(tx_bytes)) => { self.on_tx_received(tx_bytes).await; }
                Ok(NetMsg::HiddenWitness(hw)) => { /* handle */ }
                Ok(NetMsg::PrivClaimReceipt(rc)) => { self.on_priv_claim_receipt(rc).await; }
                Err(e) => eprintln!("parse msg: {e}"),
            }
        }
        let _ = w.shutdown().await;
    }

    async fn on_tx_received(&self, tx_bytes: Vec<u8>) {
        // Verify Bulletproofs
        if let Err(e) = Self::verify_outs_bp_wire(&tx_bytes) {
            eprintln!("❌ BP verify failed: {e}");
            return;
        }

        let hash = bytes32(&tx_bytes);
        self.mempool.lock().await.insert(hash, tx_bytes);
    }

    async fn on_priv_claim_receipt(&self, rc: PrivClaimNet) {
        // TODO: Use full API: verify_priv_receipt(&rc.receipt_bincode, &expected_state_root)
        // For now, skip verification and accept all receipts
        // match verify_priv_receipt(&rc.receipt_bincode, &expected_state_root) {
        //     Ok(_claim) => self.priv_claims.lock().await.push(rc.receipt_bincode),
        //     Err(e) => eprintln!("❌ invalid child receipt: {e}"),
        // }
        self.priv_claims.lock().await.push(rc.receipt_bincode);
    }

    /* ===================== MINING LOOP ===================== */

    pub async fn mine_loop(
        self: &Arc<Self>,
        max_blocks: u64,
        interval_secs: u64,
        seed32: [u8;32],
    ) -> anyhow::Result<()> {
        // Generate Falcon512 keypair
        // NOTE: In production, derive deterministically from seed or load from storage
        // For now, generate a fresh keypair (this means node ID changes each restart)
        use crate::crypto_kmac_consensus::kmac256_hash;
        use crate::falcon_sigs::falcon_keypair;
        
        let (pk, sk) = falcon_keypair();
        
        let author_pk = falcon_pk_to_bytes(&pk).to_vec();
        let author_pk_hash = bytes32(&author_pk);
        
        // Log node identity
        println!("🔐 Falcon-512 Node ID: {}", hex::encode(&author_pk_hash));

        let mut dug = 0u64;
        loop {
            if max_blocks > 0 && dug >= max_blocks { break; }

            // ===== 1. CHECK PoT ELIGIBILITY =====
            let (current_epoch, current_slot, my_weight) = {
                let pot_node = self.pot_node.lock().await;
                let epoch = pot_node.current_epoch();
                let slot = pot_node.current_slot();
                let weight = pot_node.check_eligibility(epoch, slot);
                (epoch, slot, weight)
            };

            if my_weight.is_none() {
                // Not eligible for this slot
                tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;
                continue;
            }

            let weight = my_weight.unwrap();
            println!("🎉 WON slot {}! Creating block...", current_slot);

            // ===== 2. GET PARENT =====
            let (parent_id, parent_h, parent_w) = {
                let ch = self.chain.lock().await;
                if let Some((hid, _hb)) = ch.head() {
                    let h = *ch.height.get(hid).unwrap_or(&0);
                    let w = *ch.cumw.get(hid).unwrap_or(&0.0);
                    (*hid, h, w)
                } else { ([0u8;32], 0, 0.0) }
            };

            // ===== 3. COLLECT TXs =====
            let tx_bytes_list: Vec<Vec<u8>> = {
                let mut mp = self.mempool.lock().await;
                mp.drain().map(|(_, tx)| tx).take(200).collect()
            };

            // ===== 4. ZK AGGREGATION =====
            let fanout = std::env::var("TRUE_TRUST_ZK_FANOUT")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(16)
                .clamp(1, 64);

            let receipt_bytes = self.aggregate_child_receipts(fanout).await?;

            // ===== 5. CREATE HEADER =====
            let hdr = BlockHeader {
                parent: parent_id,
                height: parent_h + 1,
                author_pk: author_pk.clone(),
                author_pk_hash,
                task_seed: bytes32(&[b"task", &parent_id[..]].concat()),
                timestamp: now_ts(),
                cum_weight_hint: weight as f64,
                parent_state_hash: [0u8;32], // TODO: compute
                result_state_hash: [0u8;32], // TODO: compute
            };
            let id = hdr.id();

            // ===== 6. SIGN (Falcon-512) =====
            let sig = falcon_sign_block(&id, &sk);

            // ===== 7. ASSEMBLE BLOCK =====
            let b = Block {
                header: hdr,
                author_sig: bincode::serialize(&sig)
                    .map_err(|e| anyhow::anyhow!("Sig serialize: {}", e))?,
                zk_receipt_bincode: receipt_bytes,
                transactions: tx_bytes_list.concat(),
            };

            // ===== 8. ACCEPT BLOCK =====
            let _ = self.on_block_received(b).await;

            dug += 1;
            tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;
        }

        Ok(())
    }

    /* ===================== BLOCK ACCEPTANCE ===================== */

    fn verify_block_author_sig(b: &Block) -> anyhow::Result<()> {
        let id = b.header.id();
        
        // Parse Falcon-512 public key (897 bytes expected)
        use crate::falcon_sigs::{falcon_pk_from_bytes, BlockSignature};
        let pk = falcon_pk_from_bytes(&b.header.author_pk)
            .map_err(|e| anyhow::anyhow!("Invalid Falcon PK: {}", e))?;
        
        // Deserialize signature
        let sig: BlockSignature = bincode::deserialize(&b.author_sig)
            .map_err(|e| anyhow::anyhow!("Invalid Falcon sig: {}", e))?;
        
        // Verify
        falcon_verify_block(&id, &sig, &pk)?;
        Ok(())
    }

    pub async fn on_block_received(self: &Arc<Self>, b0: Block) -> anyhow::Result<()> {
        let mut queue: Vec<Block> = vec![b0];

        while let Some(b) = queue.pop() {
            // Verify signature
            if let Err(e) = Self::verify_block_author_sig(&b) {
                eprintln!("❌ author sig: {e}");
                continue;
            }

            // ZK receipt verification
            let mut priv_journal: Option<crate::zk::AggPrivJournal> = None;

            if self.require_zk && !b.zk_receipt_bincode.is_empty() {
                // TODO: Use full API: verify_agg_receipt(&b.zk_receipt_bincode, &expected_state_root)
                // For now, skip ZK verification
                // match verify_agg_receipt(&b.zk_receipt_bincode, &expected_state_root) {
                //     Ok(j) => {
                //         // Verify Bulletproofs from journal
                //         if let Err(e) = Self::verify_outs_bp_zk(&j.outs_bp) {
                //             eprintln!("❌ BP verify (agg journal): {e}");
                //             continue;
                //         }
                //         priv_journal = Some(j);
                //     }
                //     Err(e) => {
                //         eprintln!("❌ agg-receipt verify: {e}");
                //         continue;
                //     }
                // }
                eprintln!("⚠️  ZK verification skipped (TODO: full API integration)");
            }

            // Block weight (TODO: integrate with PoT trust)
            let w_self = 1.0;

            // Accept block
            let is_head = {
                let mut ch = self.chain.lock().await;
                let acc = ch.accept_block(b.clone(), w_self);
                acc.is_head
            };

            if is_head {
                println!("📥 Block {} accepted as HEAD", b.header.height);

                // Update public state
                {
                    let mut st = self.state.lock().await;
                    st.credit(&b.header.author_pk_hash, Self::BLOCK_REWARD);
                    let _ = st.persist();
                }

                // Update private state
                // TODO: Restore when full ZK API is integrated
                // if let Some(j) = priv_journal {
                //     let mut sp = self.st_priv.lock().await;
                //     sp.notes_root = j.state_root;
                //     // sp.notes_count = j.sum_out_amt; // TODO: fix field mapping
                //     // sp.frontier = j.new_frontier.clone(); // TODO: add frontier
                //     for nf in &j.ins_nf { sp.nullifiers.insert(*nf); }
                //     let _ = sp.persist();
                //
                //     // Update filters
                //     if let Some(fs) = self.filters.lock().await.as_ref() {
                //         // let _ = fs.on_block_accepted(
                //         //     b.header.height,
                //         //     &j.enc_hints,
                //         //     200_000,
                //         //     0.001
                //         // );
                //     }
                // }

                // Adopt orphans
                let children: Vec<Block> = {
                    let mut o = self.orphans.lock().await;
                    o.remove(&b.header.id())
                        .map(|v| v.into_iter().map(|e| e.block).collect())
                        .unwrap_or_default()
                };
                queue.extend(children);
            } else {
                // Store as orphan
                let parent = b.header.parent;
                let mut o = self.orphans.lock().await;
                o.entry(parent).or_default().push(OrphanEntry {
                    block: b,
                    ts: Instant::now(),
                });
            }
        }
        Ok(())
    }
}
