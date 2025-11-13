//! RANDAO beacon - commit-reveal VRF

use super::types::{NodeId, Proposal};
use crate::crypto::kmac256_hash;
use std::collections::HashMap;

/// RANDAO beacon state
#[derive(Debug, Clone)]
pub struct RandaoBeacon {
    pub epoch: u64,
    pub commitments: HashMap<NodeId, [u8; 32]>,
    pub reveals: HashMap<NodeId, [u8; 32]>,
    pub beacon_value: Option<[u8; 32]>,
}

impl RandaoBeacon {
    pub fn new(epoch: u64) -> Self {
        Self {
            epoch,
            commitments: HashMap::new(),
            reveals: HashMap::new(),
            beacon_value: None,
        }
    }

    /// Submit commitment: hash(secret)
    pub fn commit(&mut self, who: &NodeId, commitment: [u8; 32]) -> anyhow::Result<()> {
        if self.commitments.contains_key(who) {
            anyhow::bail!("Already committed");
        }
        self.commitments.insert(*who, commitment);
        Ok(())
    }

    /// Reveal secret
    pub fn reveal(&mut self, who: &NodeId, secret: [u8; 32]) -> anyhow::Result<()> {
        let commitment = self.commitments.get(who)
            .ok_or_else(|| anyhow::anyhow!("No commitment found"))?;
        
        // Verify: hash(secret) == commitment
        let computed = kmac256_hash(b"RANDAO.COMMIT", &[&secret]);
        if computed != *commitment {
            anyhow::bail!("Invalid reveal");
        }
        
        self.reveals.insert(*who, secret);
        Ok(())
    }

    /// Finalize beacon value by XORing all reveals
    pub fn finalize(&mut self) -> anyhow::Result<[u8; 32]> {
        if self.reveals.is_empty() {
            anyhow::bail!("No reveals");
        }

        let mut beacon = [0u8; 32];
        for secret in self.reveals.values() {
            for i in 0..32 {
                beacon[i] ^= secret[i];
            }
        }

        self.beacon_value = Some(beacon);
        Ok(beacon)
    }

    /// Get beacon value for (epoch, slot)
    pub fn value(&self, epoch: u64, slot: u64) -> [u8; 32] {
        let base = self.beacon_value.unwrap_or([0u8; 32]);
        kmac256_hash(b"BEACON.SLOT", &[&base, &epoch.to_le_bytes(), &slot.to_le_bytes()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;

    #[test]
    fn test_commit_reveal() {
        let mut beacon = RandaoBeacon::new(1);
        let node1 = [1u8; 32];
        
        let mut secret = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut secret);
        
        let commitment = kmac256_hash(b"RANDAO.COMMIT", &[&secret]);
        beacon.commit(&node1, commitment).unwrap();
        beacon.reveal(&node1, secret).unwrap();
        
        let value = beacon.finalize().unwrap();
        assert_eq!(value, secret);
    }
}
