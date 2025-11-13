//! Public blockchain state: balances, trust, keyset, nonces

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use crate::core::Hash32;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct State {
    pub balances: HashMap<Hash32, u64>,
    pub trust: HashMap<Hash32, f64>,
    pub keyset: HashMap<Hash32, Vec<u8>>,
    pub nonces: HashMap<Hash32, u64>,

    #[serde(skip)]
    pub path: Option<PathBuf>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            balances: HashMap::new(),
            trust: HashMap::new(),
            keyset: HashMap::new(),
            nonces: HashMap::new(),
            path: None,
        }
    }
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(p: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = p.as_ref();
        if !path.exists() {
            let mut s = Self::default();
            s.path = Some(path.to_path_buf());
            s.persist()?;
            return Ok(s);
        }
        let buf = std::fs::read(path)?;
        let mut s: Self = serde_json::from_slice(&buf)?;
        s.path = Some(path.to_path_buf());
        Ok(s)
    }

    pub fn save(&self, p: impl AsRef<Path>) -> anyhow::Result<()> {
        let buf = serde_json::to_vec_pretty(self)?;
        std::fs::write(p.as_ref(), buf)?;
        Ok(())
    }

    pub fn persist(&self) -> anyhow::Result<()> {
        if let Some(ref p) = self.path {
            self.save(p)?;
        }
        Ok(())
    }
    
    /// Compute Merkle root of current state
    pub fn compute_root(&self) -> crate::core::Hash32 {
        use crate::core::shake256_bytes;
        
        // Serialize all state components
        let balances_bytes = bincode::serialize(&self.balances).unwrap_or_default();
        let trust_bytes = bincode::serialize(&self.trust).unwrap_or_default();
        let keyset_bytes = bincode::serialize(&self.keyset).unwrap_or_default();
        let nonces_bytes = bincode::serialize(&self.nonces).unwrap_or_default();
        
        // Combine and hash
        let mut combined = Vec::new();
        combined.extend_from_slice(&balances_bytes);
        combined.extend_from_slice(&trust_bytes);
        combined.extend_from_slice(&keyset_bytes);
        combined.extend_from_slice(&nonces_bytes);
        
        shake256_bytes(&combined)
    }

    pub fn credit(&mut self, who: &Hash32, amt: u64) {
        *self.balances.entry(*who).or_insert(0) += amt;
    }

    pub fn get_trust(&self, who: &Hash32) -> f64 {
        *self.trust.get(who).unwrap_or(&0.0)
    }

    pub fn set_trust(&mut self, who: &Hash32, val: f64) {
        self.trust.insert(*who, val);
    }
}
