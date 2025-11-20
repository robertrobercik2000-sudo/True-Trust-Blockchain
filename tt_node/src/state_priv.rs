// state_priv.rs
#![forbid(unsafe_code)]

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use crate::core::Hash32; // <– ważne, spójny typ

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct StatePriv {
    pub notes_root: Hash32,
    pub notes_count: u64,
    pub frontier: Vec<Hash32>,
    pub nullifiers: HashSet<Hash32>,

    #[serde(skip)]
    pub path: Option<PathBuf>,
}

impl StatePriv {
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

    pub fn insert_nullifier(&mut self, nf: Hash32) -> bool {
        self.nullifiers.insert(nf)
    }

    pub fn has_nullifier(&self, nf: &Hash32) -> bool {
        self.nullifiers.contains(nf)
    }
}