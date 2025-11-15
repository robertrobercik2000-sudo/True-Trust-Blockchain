#![forbid(unsafe_code)]

//! Trust-based consensus primitives (from user's production code)

use std::collections::HashMap;
use crate::core::Hash32;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Trust {
    pub map: HashMap<Hash32, f64>,
}

impl Trust {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn get(&self, who: &Hash32) -> f64 {
        *self.map.get(who).unwrap_or(&0.0)
    }

    pub fn set(&mut self, who: &Hash32, val: f64) {
        self.map.insert(*who, val);
    }

    pub fn decay(&mut self, factor: f64) {
        for v in self.map.values_mut() {
            *v *= factor;
        }
    }
}

impl Default for Trust {
    fn default() -> Self {
        Self::new()
    }
}
