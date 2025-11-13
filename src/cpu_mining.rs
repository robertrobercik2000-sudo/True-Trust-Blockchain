//! CPU Mining Algorithm - Optimized for OLD CPUs
//! 
//! Hybrid: PoT (2/3) + PoS (1/3) + Micro PoW
//! Algorithm: RandomX-lite (no AVX2, no JIT - pure interpreter)
//! 
//! Features:
//! - CPU cache-friendly (L1/L2/L3)
//! - Integer + AES operations (old CPU support)
//! - No GPU advantage
//! - Memory-hard (anti-ASIC)

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use crate::core::Hash32;
use crate::pot::{Q, ONE_Q, q_from_ratio, qmul, qadd};
use crate::cpu_proof::{MicroPowParams, PowProof, ProofMetrics};
use tiny_keccak::{Hasher, Shake};

/// Hybrid consensus parameters
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HybridConsensusParams {
    /// PoT weight (trust) - recommended: 2/3
    pub pot_weight: f64,
    
    /// PoS weight (stake) - recommended: 1/3
    pub pos_weight: f64,
    
    /// Minimum stake required (in base units)
    pub min_stake: u64,
    
    /// Micro PoW difficulty (bits)
    pub pow_difficulty_bits: u8,
    
    /// Trust reward for proof generation
    pub proof_trust_reward: f64,
    
    /// RandomX-lite scratchpad size (KB) - default: 256 KB (old CPU friendly)
    pub scratchpad_kb: usize,
}

impl Default for HybridConsensusParams {
    fn default() -> Self {
        Self {
            pot_weight: 2.0 / 3.0,  // 66.67%
            pos_weight: 1.0 / 3.0,  // 33.33%
            min_stake: 1_000_000,   // 1M base units
            pow_difficulty_bits: 16,
            proof_trust_reward: 0.01,
            scratchpad_kb: 256,     // 256 KB - fits in L2 cache of old CPUs
        }
    }
}

/// RandomX-lite scratchpad (simplified for old CPUs)
pub struct Scratchpad {
    data: Vec<u64>,
    size: usize,
}

impl Scratchpad {
    /// Create new scratchpad
    pub fn new(size_kb: usize) -> Self {
        let size = (size_kb * 1024) / 8; // u64 elements
        Self {
            data: vec![0u64; size],
            size,
        }
    }
    
    /// Initialize with seed
    pub fn init(&mut self, seed: &Hash32) {
        let mut sh = Shake::v256();
        sh.update(b"RANDOMX_LITE");
        sh.update(seed);
        
        // Fill scratchpad with pseudo-random data
        for i in 0..self.size {
            let mut buf = [0u8; 8];
            sh.finalize(&mut buf);
            self.data[i] = u64::from_le_bytes(buf);
            
            // Re-init for next chunk
            sh = Shake::v256();
            sh.update(&buf);
        }
    }
    
    /// Read from scratchpad (with cache-line alignment)
    #[inline]
    fn read(&self, index: usize) -> u64 {
        self.data[index % self.size]
    }
    
    /// Write to scratchpad
    #[inline]
    fn write(&mut self, index: usize, value: u64) {
        self.data[index % self.size] = value;
    }
}

/// RandomX-lite virtual machine (interpreter-only, no JIT)
pub struct RandomXLite {
    registers: [u64; 8],
    scratchpad: Scratchpad,
    iterations: u32,
}

impl RandomXLite {
    /// Create new VM with seed
    pub fn new(seed: &Hash32, scratchpad_kb: usize, iterations: u32) -> Self {
        let mut scratchpad = Scratchpad::new(scratchpad_kb);
        scratchpad.init(seed);
        
        // Initialize registers with seed
        let mut registers = [0u64; 8];
        for i in 0..8 {
            let offset = i * 4;
            if offset + 4 <= seed.len() {
                registers[i] = u32::from_le_bytes([
                    seed[offset],
                    seed[offset + 1],
                    seed[offset + 2],
                    seed[offset + 3],
                ]) as u64;
            }
        }
        
        Self {
            registers,
            scratchpad,
            iterations,
        }
    }
    
    /// Execute program (simplified instruction set for old CPUs)
    pub fn execute(&mut self, input: &[u8]) -> Hash32 {
        // Mix input into registers
        for (i, chunk) in input.chunks(8).enumerate() {
            let mut buf = [0u8; 8];
            buf[..chunk.len()].copy_from_slice(chunk);
            let val = u64::from_le_bytes(buf);
            self.registers[i % 8] ^= val;
        }
        
        // Execute random operations
        for round in 0..self.iterations {
            self.execute_round(round);
        }
        
        // Final hash
        self.finalize()
    }
    
    /// Execute one round (CPU-friendly operations)
    #[inline]
    fn execute_round(&mut self, round: u32) {
        let r0 = self.registers[0];
        let r1 = self.registers[1];
        
        // Instruction 1: ADD + XOR (old CPU friendly)
        let addr = ((r0 as usize) ^ (round as usize)) % self.scratchpad.size;
        let mem_val = self.scratchpad.read(addr);
        self.registers[2] = self.registers[2].wrapping_add(mem_val);
        
        // Instruction 2: ROT + MUL (uses integer units)
        self.registers[3] = self.registers[3].rotate_left(r1 as u32).wrapping_mul(r0 | 1);
        
        // Instruction 3: Memory store
        let store_addr = ((self.registers[2] as usize) ^ (round as usize)) % self.scratchpad.size;
        self.scratchpad.write(store_addr, self.registers[3]);
        
        // Instruction 4: AES-like mix (if available, else XOR cascade)
        self.registers[4] ^= self.registers[5];
        self.registers[5] = self.registers[5].wrapping_add(self.registers[6]);
        self.registers[6] ^= self.registers[7];
        self.registers[7] = self.registers[7].rotate_right(17);
        
        // Rotate registers
        let tmp = self.registers[0];
        for i in 0..7 {
            self.registers[i] = self.registers[i + 1];
        }
        self.registers[7] = tmp;
    }
    
    /// Finalize and produce hash
    fn finalize(&self) -> Hash32 {
        let mut sh = Shake::v256();
        sh.update(b"FINAL");
        
        // Mix all registers
        for r in &self.registers {
            sh.update(&r.to_le_bytes());
        }
        
        // Mix scratchpad samples (every 32nd element for speed)
        for i in (0..self.scratchpad.size).step_by(32) {
            sh.update(&self.scratchpad.data[i].to_le_bytes());
        }
        
        let mut out = [0u8; 32];
        sh.finalize(&mut out);
        out
    }
}

/// Hybrid mining: PoT + PoS + Micro PoW
#[derive(Clone, Debug)]
pub struct HybridMiningTask {
    /// Block data to mine
    pub block_data: Vec<u8>,
    
    /// Validator stake (Q32.32)
    pub stake_q: Q,
    
    /// Validator trust (Q32.32)
    pub trust_q: Q,
    
    /// Proof metrics (for trust building)
    pub proof_metrics: ProofMetrics,
    
    /// Consensus params
    pub params: HybridConsensusParams,
}

/// Mining result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MiningResult {
    pub pow_proof: PowProof,
    pub randomx_hash: Hash32,
    pub final_weight: f64,
    pub trust_earned: f64,
}

impl HybridMiningTask {
    /// Mine a block (hybrid PoT+PoS+PoW)
    pub fn mine(&self) -> Option<MiningResult> {
        // 1. Check minimum stake
        if !self.check_min_stake() {
            return None;
        }
        
        // 2. Compute base weight: (2/3)×trust + (1/3)×stake
        let weight = self.compute_hybrid_weight();
        
        // 3. RandomX-lite hash (CPU-friendly)
        let randomx_hash = self.compute_randomx_hash();
        
        // 4. Micro PoW (using RandomX output as seed)
        let pow_params = MicroPowParams {
            difficulty_bits: self.params.pow_difficulty_bits,
            max_iterations: 1_000_000,
        };
        
        let pow_proof = crate::cpu_proof::mine_micro_pow(
            &randomx_hash,
            &pow_params,
        )?;
        
        // 5. Calculate trust earned from proof generation
        let trust_earned = crate::cpu_proof::calculate_proof_trust_reward(
            &self.proof_metrics,
            0.3,  // BP weight
            0.4,  // ZK weight
            0.2,  // PoW weight
            self.params.proof_trust_reward,
        );
        
        Some(MiningResult {
            pow_proof,
            randomx_hash,
            final_weight: weight,
            trust_earned,
        })
    }
    
    /// Check minimum stake requirement
    fn check_min_stake(&self) -> bool {
        // Convert Q32.32 to u64
        let stake_int = (self.stake_q >> 32) as u64;
        stake_int >= self.params.min_stake
    }
    
    /// Compute hybrid weight: (2/3)×trust + (1/3)×stake
    fn compute_hybrid_weight(&self) -> f64 {
        let trust_f64 = (self.trust_q as f64) / (ONE_Q as f64);
        let stake_f64 = (self.stake_q as f64) / (ONE_Q as f64);
        
        self.params.pot_weight * trust_f64 + self.params.pos_weight * stake_f64
    }
    
    /// Compute RandomX-lite hash (optimized for old CPUs)
    fn compute_randomx_hash(&self) -> Hash32 {
        // Seed from block data
        let mut seed = [0u8; 32];
        let mut sh = Shake::v256();
        sh.update(b"SEED");
        sh.update(&self.block_data);
        sh.finalize(&mut seed);
        
        // RandomX-lite VM (256 KB scratchpad, 1024 iterations)
        let mut vm = RandomXLite::new(
            &seed,
            self.params.scratchpad_kb,
            1024,
        );
        
        vm.execute(&self.block_data)
    }
}

/// Verify hybrid mining result
pub fn verify_mining_result(
    block_data: &[u8],
    result: &MiningResult,
    params: &HybridConsensusParams,
) -> bool {
    // 1. Verify micro PoW
    let pow_params = MicroPowParams {
        difficulty_bits: params.pow_difficulty_bits,
        max_iterations: 1_000_000,
    };
    
    if !crate::cpu_proof::verify_micro_pow(&result.randomx_hash, &result.pow_proof, &pow_params) {
        return false;
    }
    
    // 2. Verify RandomX hash (recompute)
    let mut seed = [0u8; 32];
    let mut sh = Shake::v256();
    sh.update(b"SEED");
    sh.update(block_data);
    sh.finalize(&mut seed);
    
    let mut vm = RandomXLite::new(&seed, params.scratchpad_kb, 1024);
    let computed_hash = vm.execute(block_data);
    
    computed_hash == result.randomx_hash
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_scratchpad() {
        let mut sp = Scratchpad::new(256); // 256 KB
        let seed = [0x42u8; 32];
        sp.init(&seed);
        
        // Check initialized
        assert_ne!(sp.read(0), 0);
        
        // Write/read
        sp.write(100, 0xDEADBEEF);
        assert_eq!(sp.read(100), 0xDEADBEEF);
    }
    
    #[test]
    fn test_randomx_lite() {
        let seed = [0x55u8; 32];
        let mut vm = RandomXLite::new(&seed, 256, 1024);
        
        let input = b"test_block_data";
        let hash1 = vm.execute(input);
        
        // Different input should produce different hash
        let mut vm2 = RandomXLite::new(&seed, 256, 1024);
        let hash2 = vm2.execute(b"different_data");
        
        assert_ne!(hash1, hash2);
    }
    
    #[test]
    fn test_hybrid_mining() {
        let task = HybridMiningTask {
            block_data: b"test_block".to_vec(),
            stake_q: ONE_Q * 2_000_000, // 2M stake (> min 1M)
            trust_q: ONE_Q / 2,  // 0.5 trust
            proof_metrics: ProofMetrics {
                bp_generated: 5,
                zk_generated: 2,
                cpu_time_ms: 1000,
                pow_iterations: 50000,
            },
            params: HybridConsensusParams::default(),
        };
        
        let result = task.mine();
        assert!(result.is_some(), "Mining should succeed with sufficient stake");
        
        if let Some(r) = result {
            println!("✅ Weight: {:.4}", r.final_weight);
            println!("✅ Trust earned: {:.4}", r.trust_earned);
            println!("✅ PoW iterations: {}", r.pow_proof.iterations);
            
            // Verify
            assert!(verify_mining_result(&task.block_data, &r, &task.params));
        }
    }
    
    #[test]
    fn test_min_stake_rejection() {
        let task = HybridMiningTask {
            block_data: b"test_block".to_vec(),
            stake_q: ONE_Q * 500_000, // 500k stake (< min 1M) - FAIL
            trust_q: ONE_Q,  // 1.0 trust (max)
            proof_metrics: ProofMetrics::default(),
            params: HybridConsensusParams::default(),
        };
        
        let result = task.mine();
        assert!(result.is_none(), "Mining should fail with insufficient stake");
        println!("✅ Correctly rejected low stake");
    }
}
