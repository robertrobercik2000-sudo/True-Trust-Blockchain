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
