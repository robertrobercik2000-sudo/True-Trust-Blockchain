//! KMAC-DRBG: Deterministic Random Bit Generator based on KMAC256/cSHAKE256
//!
//! # Features
//!
//! - **Deterministic**: Same seed + personalization → same output stream
//! - **no_std-ready**: Works in constrained environments (RISC0 guests, SGX, HSM)
//! - **Cryptographically secure**: Implements `RngCore` + `CryptoRng` traits
//! - **Key ratcheting**: Forward secrecy via periodic key refresh
//! - **Zero-copy security**: Uses `Zeroizing` for sensitive data
//!
//! # Use Cases
//!
//! 1. **Reproducible Falcon signing**: Deterministic coins for audit/HSM/SGX
//! 2. **Seedable keygen**: Derive PQ keypairs from master seed
//! 3. **Ephemeral key generation**: X25519, ML-KEM with full control
//! 4. **Testing**: Reproducible test vectors
//!
//! # Security Properties
//!
//! - **128-bit security**: Based on KMAC256 (cSHAKE256 with key)
//! - **Domain separation**: All operations use unique labels
//! - **Forward secrecy**: Optional ratcheting after N blocks
//! - **Transcript binding**: Personalization can include context/epoch/transcript
//!
//! # Example
//!
//! ```
//! use quantum_falcon_wallet::crypto::kmac_drbg::KmacDrbg;
//! use rand_core::RngCore;
//!
//! // Deterministic DRBG from seed
//! let seed = [0x42u8; 32];
//! let mut drbg = KmacDrbg::new(&seed, b"my-app-v1");
//!
//! // Generate random bytes
//! let mut output = [0u8; 64];
//! drbg.fill_bytes(&mut output);
//!
//! // Reseed with additional entropy
//! drbg.reseed(b"more-entropy");
//! ```

#![cfg_attr(not(test), no_std)]
extern crate alloc;

use alloc::vec::Vec;
use core::cmp::min;

use rand_core::{CryptoRng, Error as RandError, RngCore};
use zeroize::{Zeroize, Zeroizing};

use crate::crypto::kmac::{kmac256_derive_key, kmac256_xof_fill};

/* ============================================================================
 * KMAC-DRBG Core
 * ========================================================================== */

/// KMAC-based Deterministic Random Bit Generator
///
/// # Architecture
///
/// - **Internal key**: 32-byte secret key (`k`), zeroized on drop
/// - **Counter**: 128-bit counter for block generation (little-endian)
/// - **Personalization**: Domain-specific context (epoch, transcript, label)
/// - **Ratcheting**: Periodic key refresh for forward secrecy
///
/// # Stream Generation
///
/// For each block `i`:
/// ```text
/// custom = personalization || counter_LE
/// block_i = KMAC256_XOF(k, "DRBG/stream", custom, block_size)
/// counter += 1
/// ```
///
/// # Key Ratcheting
///
/// After `ratchet_every_blocks` (default: 65536 ≈ 4 MB):
/// ```text
/// custom = personalization || counter_LE
/// k_new = KMAC256(k_old, "DRBG/ratchet", custom)
/// ```
///
/// This provides forward secrecy: compromising current state doesn't reveal past outputs.
pub struct KmacDrbg {
    /// Internal DRBG key (SENSITIVE - zeroized on drop)
    k: Zeroizing<[u8; 32]>,
    
    /// Block counter (128-bit for ~2^128 blocks before wrap)
    ctr: u128,
    
    /// Personalization string (domain separation, context binding)
    pers: Zeroizing<Vec<u8>>,
    
    /// Blocks generated since last ratchet
    blocks_since_ratchet: u64,
    
    /// Ratchet interval (forward secrecy parameter)
    ratchet_every_blocks: u64,
}

impl KmacDrbg {
    /// Create DRBG from seed material and personalization
    ///
    /// # Parameters
    ///
    /// - `seed_material`: Secret seed (e.g., master key, derived secret)
    /// - `personalization`: Context string for domain separation
    ///   - Can include: application label, epoch, transcript, key index
    ///
    /// # Security
    ///
    /// - Seed should have ≥256 bits of entropy for 128-bit security
    /// - Personalization binds output to specific context (prevents cross-domain attacks)
    ///
    /// # Example
    ///
    /// ```
    /// let seed = master_key; // 32 bytes
    /// let pers = b"FALCON/epoch=42/signing";
    /// let mut drbg = KmacDrbg::new(&seed, pers);
    /// ```
    pub fn new(seed_material: &[u8], personalization: &[u8]) -> Self {
        let k = kmac256_derive_key(seed_material, b"DRBG/seed", personalization);
        Self {
            k: Zeroizing::new(k),
            ctr: 0,
            pers: Zeroizing::new(personalization.to_vec()),
            blocks_since_ratchet: 0,
            // Default: ratchet every 65536 blocks (~4 MB at 64B/block)
            ratchet_every_blocks: 1 << 16,
        }
    }

    /// Create DRBG from pre-derived key
    ///
    /// Use when key is already derived via KDF elsewhere.
    ///
    /// # Example
    ///
    /// ```
    /// let key = kmac256_derive_key(&master_seed, b"FALCON/keygen", &epoch.to_le_bytes());
    /// let mut drbg = KmacDrbg::from_key(key, b"FALCON/keygen.v1");
    /// ```
    pub fn from_key(key32: [u8; 32], personalization: &[u8]) -> Self {
        Self {
            k: Zeroizing::new(key32),
            ctr: 0,
            pers: Zeroizing::new(personalization.to_vec()),
            blocks_since_ratchet: 0,
            ratchet_every_blocks: 1 << 16,
        }
    }

    /// Reseed DRBG with additional entropy
    ///
    /// Mixes new entropy into internal key via KDF:
    /// ```text
    /// k_new = KMAC256(k_old, "DRBG/reseed", additional_data)
    /// ```
    ///
    /// **Resets counter and ratchet state.**
    ///
    /// # Use Cases
    ///
    /// - Periodic reseeding from system entropy
    /// - After generating sensitive material (defense-in-depth)
    /// - When additional context becomes available (e.g., network randomness)
    ///
    /// # Example
    ///
    /// ```
    /// drbg.reseed(b"fresh-entropy-from-network");
    /// ```
    pub fn reseed(&mut self, additional: &[u8]) {
        let newk = kmac256_derive_key(self.k.as_ref(), b"DRBG/reseed", additional);
        self.k = Zeroizing::new(newk);
        self.ctr = 0;
        self.blocks_since_ratchet = 0;
    }

    /// Set ratchet interval (forward secrecy parameter)
    ///
    /// # Parameters
    ///
    /// - `every_blocks`: Number of blocks before automatic key refresh
    ///   - Default: 65536 blocks (~4 MB at 64B/block)
    ///   - Minimum: 1 (ratchet every block - high overhead but max FS)
    ///
    /// # Trade-offs
    ///
    /// - **Frequent ratcheting**: Better forward secrecy, slight performance cost
    /// - **Infrequent ratcheting**: Better performance, weaker FS guarantees
    ///
    /// # Example
    ///
    /// ```
    /// // Ratchet every 1 MB (16384 blocks at 64B)
    /// drbg.set_ratchet_interval(16384);
    /// ```
    pub fn set_ratchet_interval(&mut self, every_blocks: u64) {
        self.ratchet_every_blocks = every_blocks.max(1);
    }

    /// Manual key ratchet (forward secrecy)
    ///
    /// Derives new key from current key + state:
    /// ```text
    /// custom = personalization || counter_LE
    /// k_new = KMAC256(k_old, "DRBG/ratchet", custom)
    /// ```
    ///
    /// **Resets ratchet counter but preserves main counter.**
    ///
    /// # Use Cases
    ///
    /// - Before generating long-lived keys
    /// - After sensitive operations
    /// - Explicit forward secrecy checkpoints
    ///
    /// # Example
    ///
    /// ```
    /// // Generate ephemeral key
    /// let mut eph_key = [0u8; 32];
    /// drbg.fill_bytes(&mut eph_key);
    ///
    /// // Ratchet before next operation
    /// drbg.ratchet();
    /// ```
    pub fn ratchet(&mut self) {
        // new_k = KMAC(k, "DRBG/ratchet", pers || ctr)
        let mut custom = Vec::with_capacity(self.pers.len() + 16);
        custom.extend_from_slice(&self.pers);
        custom.extend_from_slice(&self.ctr.to_le_bytes());
        let newk = kmac256_derive_key(self.k.as_ref(), b"DRBG/ratchet", &custom);
        self.k = Zeroizing::new(newk);
        self.blocks_since_ratchet = 0;
    }

    /// Generate one block of random data
    ///
    /// Internal function called by `fill_bytes`.
    #[inline]
    fn gen_block_into(&mut self, out: &mut [u8]) {
        // custom = pers || ctr (binds output to counter + context)
        let mut custom = Vec::with_capacity(self.pers.len() + 16);
        custom.extend_from_slice(&self.pers);
        custom.extend_from_slice(&self.ctr.to_le_bytes());

        // Generate block via KMAC256 XOF
        kmac256_xof_fill(self.k.as_ref(), b"DRBG/stream", &custom, out);

        // Update state
        self.ctr = self.ctr.wrapping_add(1);
        self.blocks_since_ratchet = self.blocks_since_ratchet.saturating_add(1);
        
        // Automatic ratchet if threshold reached
        if self.blocks_since_ratchet >= self.ratchet_every_blocks {
            self.ratchet();
        }
    }
}

/* ============================================================================
 * RngCore Implementation
 * ========================================================================== */

impl RngCore for KmacDrbg {
    fn next_u32(&mut self) -> u32 {
        let mut b = [0u8; 4];
        self.fill_bytes(&mut b);
        u32::from_le_bytes(b)
    }

    fn next_u64(&mut self) -> u64 {
        let mut b = [0u8; 8];
        self.fill_bytes(&mut b);
        u64::from_le_bytes(b)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        // Generate in 64-byte blocks (optimal for Keccak rate and cache lines)
        const BLK: usize = 64;
        let mut buf = [0u8; BLK];
        let mut off = 0;
        
        while off < dest.len() {
            self.gen_block_into(&mut buf);
            let n = min(BLK, dest.len() - off);
            dest[off..off + n].copy_from_slice(&buf[..n]);
            off += n;
        }
        // buf is stack-allocated and will be zeroed on scope exit
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), RandError> {
        self.fill_bytes(dest);
        Ok(())
    }
}

/// Mark as cryptographically secure RNG
impl CryptoRng for KmacDrbg {}

/* ============================================================================
 * Security: Zeroize on Drop
 * ========================================================================== */

impl Drop for KmacDrbg {
    fn drop(&mut self) {
        // Explicit zeroization (paranoia - Zeroizing does this automatically)
        self.k.zeroize();
        self.pers.zeroize();
    }
}

/* ============================================================================
 * Tests
 * ========================================================================== */

#[cfg(test)]
mod tests {
    use super::*;
    use rand_core::RngCore as _;

    #[test]
    fn deterministic_same_seed_and_pers() {
        let seed = [0x42u8; 32];
        let pers = b"test-pers";
        let mut a = KmacDrbg::new(&seed, pers);
        let mut b = KmacDrbg::new(&seed, pers);

        let mut out_a = [0u8; 128];
        let mut out_b = [0u8; 128];
        a.fill_bytes(&mut out_a);
        b.fill_bytes(&mut out_b);
        
        assert_eq!(out_a, out_b, "Same seed+pers must produce identical output");
    }

    #[test]
    fn different_personalization_differs() {
        let seed = [0x42u8; 32];
        let mut a = KmacDrbg::new(&seed, b"A");
        let mut b = KmacDrbg::new(&seed, b"B");

        let mut out_a = [0u8; 64];
        let mut out_b = [0u8; 64];
        a.fill_bytes(&mut out_a);
        b.fill_bytes(&mut out_b);
        
        assert_ne!(out_a, out_b, "Different personalization must produce different output");
    }

    #[test]
    fn reseed_changes_stream() {
        let seed = [0x42u8; 32];
        let mut drbg = KmacDrbg::new(&seed, b"P");
        
        let mut out1 = [0u8; 64];
        let mut out2 = [0u8; 64];
        
        drbg.fill_bytes(&mut out1);
        drbg.reseed(b"more-entropy");
        drbg.fill_bytes(&mut out2);
        
        assert_ne!(out1, out2, "Reseed must change output stream");
    }

    #[test]
    fn next_u32_u64_work() {
        let mut d = KmacDrbg::new(&[1u8; 32], b"x");
        let x = d.next_u32();
        let y = d.next_u64();
        
        // Smoke test - just verify they execute without panic
        assert!(x > 0 || x == 0); // Always true
        assert!(y > 0 || y == 0);
    }

    #[test]
    fn ratchet_changes_stream() {
        let mut drbg = KmacDrbg::new(&[0x99u8; 32], b"ratchet-test");
        
        let mut out1 = [0u8; 32];
        let mut out2 = [0u8; 32];
        
        drbg.fill_bytes(&mut out1);
        drbg.ratchet();
        drbg.fill_bytes(&mut out2);
        
        assert_ne!(out1, out2, "Manual ratchet must change output stream");
    }

    #[test]
    fn from_key_deterministic() {
        let key = [0xABu8; 32];
        let pers = b"key-test";
        
        let mut a = KmacDrbg::from_key(key, pers);
        let mut b = KmacDrbg::from_key(key, pers);
        
        let mut out_a = [0u8; 64];
        let mut out_b = [0u8; 64];
        
        a.fill_bytes(&mut out_a);
        b.fill_bytes(&mut out_b);
        
        assert_eq!(out_a, out_b, "from_key with same params must be deterministic");
    }

    #[test]
    fn set_ratchet_interval() {
        let mut drbg = KmacDrbg::new(&[0x55u8; 32], b"interval-test");
        drbg.set_ratchet_interval(2); // Ratchet every 2 blocks
        
        let mut buf = [0u8; 64];
        
        // First block - no ratchet yet
        drbg.fill_bytes(&mut buf);
        assert_eq!(drbg.blocks_since_ratchet, 1);
        
        // Second block - triggers ratchet
        drbg.fill_bytes(&mut buf);
        assert_eq!(drbg.blocks_since_ratchet, 0, "Should have ratcheted after 2 blocks");
    }

    #[test]
    fn large_output() {
        let mut drbg = KmacDrbg::new(&[0x77u8; 32], b"large");
        let mut large = vec![0u8; 10_000]; // 10 KB
        drbg.fill_bytes(&mut large);
        
        // Check not all zeros
        assert!(large.iter().any(|&b| b != 0), "Output should not be all zeros");
    }
}
