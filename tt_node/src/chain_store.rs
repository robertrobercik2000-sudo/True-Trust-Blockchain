#![forbid(unsafe_code)]

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::core::{Hash32, Block};
use crate::consensus_weights::Weight;

/// Informacja, co się stało przy akceptacji bloku.
pub struct AcceptResult {
    pub is_new: bool,
    pub is_head: bool,
}

/// Prosty magazyn łańcucha z kumulatywną wagą (bez f64!).
#[derive(Default)]
pub struct ChainStore {
    pub blocks: HashMap<Hash32, Block>,
    pub parent: HashMap<Hash32, Hash32>,
    pub height: HashMap<Hash32, u64>,
    pub weight: HashMap<Hash32, Weight>,
    pub cumw:   HashMap<Hash32, Weight>,
    head: Option<Hash32>,
}

impl ChainStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has(&self, id: &Hash32) -> bool {
        self.blocks.contains_key(id)
    }

    pub fn get(&self, id: &Hash32) -> Option<&Block> {
        self.blocks.get(id)
    }

    pub fn head(&self) -> Option<(&Hash32, &Block)> {
        self.head
            .as_ref()
            .and_then(|hid| self.blocks.get_key_value(hid))
    }

    /// `w_self` to wynik z `compute_final_weight_q(...)` (Weight = u128).
    pub fn accept_block(&mut self, b: Block, w_self: Weight) -> AcceptResult {
        let id = b.header.id();
        let is_new = !self.blocks.contains_key(&id);

        if is_new {
            let p = b.header.parent;

            let h = if b.header.height == 0 {
                0
            } else {
                self.height.get(&p).copied().unwrap_or(0) + 1
            };

            let cw_parent = self.cumw.get(&p).copied().unwrap_or(0);
            let cw = cw_parent.saturating_add(w_self);

            self.parent.insert(id, p);
            self.height.insert(id, h);
            self.weight.insert(id, w_self);
            self.cumw.insert(id, cw);
            self.blocks.insert(id, b);
        }

        // Update HEAD (największa waga, tie-breaker: większa wysokość)
        let mut is_head = false;
        if let Some(cur) = self.head {
            let cur_cw = self.cumw.get(&cur).copied().unwrap_or(0);
            let new_cw = self.cumw.get(&id).copied().unwrap_or(0);

            if new_cw > cur_cw
                || (new_cw == cur_cw
                    && self.height.get(&id) > self.height.get(&cur))
            {
                self.head = Some(id);
                is_head = true;
            }
        } else {
            self.head = Some(id);
            is_head = true;
        }

        AcceptResult { is_new, is_head }
    }
}
