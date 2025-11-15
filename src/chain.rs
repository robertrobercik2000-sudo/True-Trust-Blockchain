#![forbid(unsafe_code)]

//! Chain storage with orphan handling and cumulative weight tracking

use std::collections::HashMap;
use crate::core::{Hash32, Block, BlockHeader};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct ChainStore {
    pub blocks: HashMap<Hash32, Block>,
    pub parent: HashMap<Hash32, Hash32>,
    pub height: HashMap<Hash32, u64>,
    pub weight: HashMap<Hash32, f64>,
    pub cumw:   HashMap<Hash32, f64>,
    head: Option<Hash32>,
}

pub struct AcceptResult {
    pub is_new: bool,
    pub is_head: bool,
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
        self.head.as_ref().and_then(|hid| self.blocks.get_key_value(hid))
    }

    pub fn accept_block(&mut self, b: Block, w_self: f64) -> AcceptResult {
        let id = b.header.id();
        let is_new = !self.blocks.contains_key(&id);

        if is_new {
            let p = b.header.parent;
            let h = if b.header.height == 0 {
                0
            } else {
                *self.height.get(&p).unwrap_or(&0) + 1
            };
            let cw_parent = *self.cumw.get(&p).unwrap_or(&0.0);
            let cw = cw_parent + w_self;

            self.parent.insert(id, p);
            self.height.insert(id, h);
            self.weight.insert(id, w_self);
            self.cumw.insert(id, cw);
            self.blocks.insert(id, b);
        }

        // Update HEAD
        let mut is_head = false;
        if let Some(cur) = self.head {
            let cur_cw = *self.cumw.get(&cur).unwrap_or(&0.0);
            let new_cw = *self.cumw.get(&id).unwrap_or(&0.0);
            if new_cw > cur_cw || (new_cw == cur_cw && self.height.get(&id) > self.height.get(&cur)) {
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
