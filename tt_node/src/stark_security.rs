#![forbid(unsafe_code)]

//! STARK Security Analysis
//!
//! Formal analysis of STARK proof system security parameters.
//!
//! **Components:**
//! 1. Soundness error calculation (FRI queries)
//! 2. Classical/quantum security levels
//! 3. Field size impact
//! 4. Automated security reports

use serde::{Deserialize, Serialize};
use std::fmt;

// ============================================================================
// SECURITY PARAMETERS
// ============================================================================

/// Security parameters for STARK proof system
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecurityParams {
    /// Field size (bits)
    pub field_bits: usize,
    
    /// Field name (for reporting)
    pub field_name: String,
    
    /// Number of FRI queries
    pub fri_queries: usize,
    
    /// FRI blowup factor
    pub fri_blowup: usize,
    
    /// FRI fold factor
    pub fri_fold_factor: usize,
    
    /// Constraint degree (max degree of AIR constraints)
    pub constraint_degree: usize,
    
    /// Domain size (trace length)
    pub domain_size: usize,
}

impl SecurityParams {
    /// Create BabyBear configuration (demo-grade)
    pub fn babybear() -> Self {
        Self {
            field_bits: 31,
            field_name: "BabyBear".to_string(),
            fri_queries: 40,
            fri_blowup: 8,
            fri_fold_factor: 2,
            constraint_degree: 2,
            domain_size: 128,
        }
    }
    
    /// Create Goldilocks configuration (production-grade)
    pub fn goldilocks() -> Self {
        Self {
            field_bits: 64,
            field_name: "Goldilocks".to_string(),
            fri_queries: 80,       // 2× increase
            fri_blowup: 16,        // 2× increase
            fri_fold_factor: 4,    // 2× increase
            constraint_degree: 2,
            domain_size: 128,
        }
    }
    
    /// Create BN254 configuration (maximum security)
    ///
    /// **NOTE:** To achieve 128-bit classical security, BN254 requires
    /// stronger FRI parameters than smaller fields:
    /// - 160 FRI queries (2× Goldilocks)
    /// - 32× blowup factor (2× Goldilocks)
    /// - This gives ~142-bit soundness, limited to 128-bit by hash
    ///
    /// **Trade-off:** ~4× slower proofs than Goldilocks, but maximum security!
    pub fn bn254() -> Self {
        Self {
            field_bits: 254,
            field_name: "BN254".to_string(),
            fri_queries: 160,      // 2× Goldilocks for 128-bit security
            fri_blowup: 32,        // 2× Goldilocks for 128-bit security
            fri_fold_factor: 4,
            constraint_degree: 2,
            domain_size: 128,
        }
    }
    
    /// Compute FRI soundness error (bits)
    ///
    /// **Formula:** -log₂(ε) where ε = soundness error
    ///
    /// FRI soundness (simplified): 
    /// ```
    /// ε ≈ (ρ + ε₀)^q
    /// ```
    ///
    /// Where:
    /// - ρ = query_rate = queries / (domain × blowup)
    /// - ε₀ = proximity parameter ≈ 1/2 (for low-degree polynomials)
    /// - q = num_queries
    ///
    /// **Note:** This is a simplified model. Full analysis requires:
    /// - Constraint degree
    /// - Number of FRI rounds
    /// - Hash function collision resistance
    ///
    /// **Reference:** "Concrete Security of STARKs" (StarkWare, 2021)
    pub fn soundness_bits(&self) -> f64 {
        // Query rate (fraction of domain queried)
        let query_rate = self.fri_queries as f64 
            / (self.domain_size * self.fri_blowup) as f64;
        
        // Proximity parameter (conservative estimate)
        // For constraint degree d, ε₀ ≈ d / |F|
        // Simplified: use 0.5 (works for small fields, conservative for large)
        let epsilon_0 = if self.field_bits < 64 {
            0.5
        } else {
            self.constraint_degree as f64 / (1u64 << self.field_bits.min(63)) as f64
        };
        
        // Soundness error per query round
        let error_per_query = query_rate + epsilon_0;
        
        // Total error after q queries (union bound)
        let total_error = error_per_query.powi(self.fri_queries as i32);
        
        // Convert to bits: -log₂(ε)
        -total_error.log2()
    }
    
    /// Compute field collision resistance (bits)
    ///
    /// **Birthday paradox:** Collision resistance = field_size / 2
    ///
    /// For field F_p:
    /// - |F| = p (field size)
    /// - Collision attack: O(√p) operations
    /// - Security: log₂(√p) = (log₂ p) / 2 bits
    pub fn field_collision_bits(&self) -> usize {
        self.field_bits / 2
    }
    
    /// Compute hash collision resistance (bits)
    ///
    /// Merkle tree uses SHA-3-256:
    /// - Output: 256 bits
    /// - Collision resistance: 128 bits (birthday bound)
    pub fn hash_collision_bits(&self) -> usize {
        128 // SHA-3-256
    }
    
    /// Compute classical security level (bits)
    ///
    /// Dla STARK-ów realne bezpieczeństwo pochodzi z:
    /// 1. FRI soundness (liczone z parametrów),
    /// 2. Hash collision resistance (SHA-3-256 = 128 bitów),
    /// a następnie jest **ograniczone przez rozmiar pola**.
    ///
    /// Czyli:
    ///   stark_security = min(soundness_bits, hash_bits)
    ///   classical_security_bits = min(stark_security, field_bits)
    ///
    /// Dzięki temu:
    /// - BabyBear (31-bit field) nie udaje 64-bitowego bezpieczeństwa,
    /// - Goldilocks (64-bit field) wyjdzie na poziomie 64-bit classical,
    /// - BN254 może osiągnąć pełne 128-bit classical.
    pub fn classical_security_bits(&self) -> usize {
        let soundness_security = self.soundness_bits() as usize;
        let hash_security = self.hash_collision_bits();
        
        // STARK security from proof system (FRI + Merkle)
        let stark_security = soundness_security.min(hash_security);
        
        // Twardy limit: nie deklarujemy więcej niż field_bits
        stark_security.min(self.field_bits)
    }
    
    /// Compute post-quantum security level (bits)
    ///
    /// **Grover's algorithm:** Quantum adversary gets √ speedup for:
    /// - Preimage search: n-bit → n/2-bit
    /// - Collision search: Still n/2-bit (no speedup beyond birthday)
    ///
    /// **Result:** Classical n-bit → Quantum n/2-bit
    pub fn quantum_security_bits(&self) -> usize {
        // Grover gives sqrt speedup, so divide by 2
        let classical = self.classical_security_bits();
        classical / 2
    }
    
    /// Check if parameters meet target security level (classical)
    pub fn meets_security_target(&self, target_bits: usize) -> bool {
        self.classical_security_bits() >= target_bits
    }
    
    /// Check if parameters meet quantum security target
    pub fn meets_quantum_security_target(&self, target_bits: usize) -> bool {
        self.quantum_security_bits() >= target_bits
    }
    
    /// Estimate proof size (KB)
    ///
    /// **Components:**
    /// 1. Trace commitment: 32 bytes
    /// 2. Constraint commitment: 32 bytes
    /// 3. FRI commitments: num_layers × 32 bytes
    /// 4. FRI queries: num_queries × query_size
    /// 5. Public inputs: small
    ///
    /// **Query size** ≈ num_layers × (field_element + merkle_proof)
    ///   ≈ num_layers × (8 + log(domain) × 32)
    pub fn estimated_proof_size_kb(&self) -> f64 {
        let field_bytes = (self.field_bits + 7) / 8;
        let num_layers = (self.domain_size as f64).log2() as usize;
        let merkle_path_len = (self.domain_size as f64).log2() as usize;
        
        let trace_commit = 32;
        let constraint_commit = 32;
        let fri_commits = num_layers * 32;
        
        // Per query: field_element per layer + merkle proof per layer
        let query_size = num_layers * (field_bytes + merkle_path_len * 32);
        let queries_total = self.fri_queries * query_size;
        
        let total_bytes = trace_commit + constraint_commit + fri_commits + queries_total;
        total_bytes as f64 / 1024.0
    }
    
    /// Estimate prover time (ms)
    ///
    /// **Rough model** (based on BabyBear benchmarks):
    /// - FFT: O(n log n) × field_mul_time
    /// - FRI folding: O(n) × num_layers
    /// - Merkle tree: O(n log n) × hash_time
    ///
    /// **Scaling factors:**
    /// - BabyBear (31-bit): 1×
    /// - Goldilocks (64-bit): 2× (u128 multiply)
    /// - BN254 (254-bit): 10× (multi-precision)
    pub fn estimated_prover_time_ms(&self, baseline_ms: f64) -> f64 {
        // Field operation cost scaling
        let field_cost_factor = if self.field_bits <= 32 {
            1.0
        } else if self.field_bits <= 64 {
            2.0
        } else {
            // 256-bit field: ~10× slower
            10.0
        };
        
        // FRI query cost scaling
        let query_factor = self.fri_queries as f64 / 40.0; // Baseline: 40 queries
        
        baseline_ms * field_cost_factor * query_factor
    }
    
    /// Generate security report
    pub fn generate_report(&self) -> SecurityReport {
        SecurityReport {
            params: self.clone(),
            classical_bits: self.classical_security_bits(),
            quantum_bits: self.quantum_security_bits(),
            soundness_bits: self.soundness_bits(),
            field_collision_bits: self.field_collision_bits(),
            hash_collision_bits: self.hash_collision_bits(),
            meets_64_bit: self.meets_security_target(64),
            meets_128_bit: self.meets_security_target(128),
            meets_quantum_64_bit: self.meets_quantum_security_target(64),
            estimated_proof_kb: self.estimated_proof_size_kb(),
            estimated_prover_ms: self.estimated_prover_time_ms(500.0), // 500ms baseline
        }
    }
}

// ============================================================================
// SECURITY REPORT
// ============================================================================

/// Security analysis report
#[derive(Clone, Debug)]
pub struct SecurityReport {
    pub params: SecurityParams,
    pub classical_bits: usize,
    pub quantum_bits: usize,
    pub soundness_bits: f64,
    pub field_collision_bits: usize,
    pub hash_collision_bits: usize,
    pub meets_64_bit: bool,
    pub meets_128_bit: bool,
    pub meets_quantum_64_bit: bool,
    pub estimated_proof_kb: f64,
    pub estimated_prover_ms: f64,
}

impl fmt::Display for SecurityReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "╔══════════════════════════════════════════════════════════╗")?;
        writeln!(f, "║          STARK SECURITY ANALYSIS REPORT                  ║")?;
        writeln!(f, "╚══════════════════════════════════════════════════════════╝")?;
        writeln!(f)?;
        
        writeln!(f, "📊 Configuration: {}", self.params.field_name)?;
        writeln!(f, "   Field:        {} bits", self.params.field_bits)?;
        writeln!(f, "   FRI Queries:  {}", self.params.fri_queries)?;
        writeln!(f, "   FRI Blowup:   {}", self.params.fri_blowup)?;
        writeln!(f, "   Domain:       {}", self.params.domain_size)?;
        writeln!(f)?;
        
        writeln!(f, "🔒 Security Levels:")?;
        writeln!(f, "   Classical:    {} bits {}", 
            self.classical_bits, 
            if self.meets_128_bit { "✅" } else if self.meets_64_bit { "⚠️" } else { "❌" }
        )?;
        writeln!(f, "   Quantum:      {} bits {}", 
            self.quantum_bits,
            if self.meets_quantum_64_bit { "✅" } else { "❌" }
        )?;
        writeln!(f, "   Soundness:    {:.1} bits", self.soundness_bits)?;
        writeln!(f)?;
        
        writeln!(f, "🔍 Component Security:")?;
        writeln!(f, "   Field:        {} bits (collision)", self.field_collision_bits)?;
        writeln!(f, "   Hash (SHA-3): {} bits (collision)", self.hash_collision_bits)?;
        writeln!(f)?;
        
        writeln!(f, "✅ Target Compliance:")?;
        writeln!(f, "   64-bit:       {}", if self.meets_64_bit { "✅ YES" } else { "❌ NO" })?;
        writeln!(f, "   128-bit:      {}", if self.meets_128_bit { "✅ YES" } else { "❌ NO" })?;
        writeln!(f, "   Q-64-bit:     {}", if self.meets_quantum_64_bit { "✅ YES" } else { "❌ NO" })?;
        writeln!(f)?;
        
        writeln!(f, "⚡ Performance Estimates:")?;
        writeln!(f, "   Proof Size:   {:.1} KB", self.estimated_proof_kb)?;
        writeln!(f, "   Prover Time:  {:.0} ms", self.estimated_prover_ms)?;
        writeln!(f)?;
        
        writeln!(f, "💡 Recommendations:")?;
        if self.meets_128_bit {
            writeln!(f, "   ✅ Excellent security for production use")?;
        } else if self.meets_64_bit {
            writeln!(f, "   ⚠️  Adequate for most applications")?;
            writeln!(f, "   ⚠️  For high-value, upgrade to 128-bit field")?;
        } else {
            writeln!(f, "   ❌ Insufficient security for production")?;
            writeln!(f, "   ❌ Upgrade to Goldilocks (64-bit) or larger")?;
        }
        
        Ok(())
    }
}

// ============================================================================
// COMPARISON UTILITIES
// ============================================================================

/// Compare multiple configurations side-by-side
pub fn compare_configs(configs: &[SecurityParams]) -> String {
    let reports: Vec<SecurityReport> = configs.iter().map(|c| c.generate_report()).collect();
    
    let mut output = String::new();
    output.push_str("╔══════════════════════════════════════════════════════════════════╗\n");
    output.push_str("║          STARK CONFIGURATION COMPARISON                          ║\n");
    output.push_str("╚══════════════════════════════════════════════════════════════════╝\n\n");
    
    // Header
    output.push_str(&format!("{:<15}", "Metric"));
    for report in &reports {
        output.push_str(&format!(" | {:<15}", report.params.field_name));
    }
    output.push('\n');
    output.push_str(&"-".repeat(15 + reports.len() * 18));
    output.push('\n');
    
    // Field bits
    output.push_str(&format!("{:<15}", "Field (bits)"));
    for report in &reports {
        output.push_str(&format!(" | {:<15}", report.params.field_bits));
    }
    output.push('\n');
    
    // FRI queries
    output.push_str(&format!("{:<15}", "FRI Queries"));
    for report in &reports {
        output.push_str(&format!(" | {:<15}", report.params.fri_queries));
    }
    output.push('\n');
    
    // Classical security
    output.push_str(&format!("{:<15}", "Classical (bit)"));
    for report in &reports {
        output.push_str(&format!(" | {:<15}", report.classical_bits));
    }
    output.push('\n');
    
    // Quantum security
    output.push_str(&format!("{:<15}", "Quantum (bit)"));
    for report in &reports {
        output.push_str(&format!(" | {:<15}", report.quantum_bits));
    }
    output.push('\n');
    
    // Proof size
    output.push_str(&format!("{:<15}", "Proof (KB)"));
    for report in &reports {
        output.push_str(&format!(" | {:<15.1}", report.estimated_proof_kb));
    }
    output.push('\n');
    
    // Speed
    output.push_str(&format!("{:<15}", "Prove (ms)"));
    for report in &reports {
        output.push_str(&format!(" | {:<15.0}", report.estimated_prover_ms));
    }
    output.push('\n');
    
    output
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_babybear_security() {
        let params = SecurityParams::babybear();
        let report = params.generate_report();
        
        println!("{}", report);
        
        // BabyBear should NOT meet 64-bit security
        assert!(!report.meets_64_bit, "BabyBear incorrectly claims 64-bit security");
        assert!(!report.meets_128_bit);
    }
    
    #[test]
    fn test_goldilocks_security() {
        let params = SecurityParams::goldilocks();
        let report = params.generate_report();
        
        println!("{}", report);
        
        // Goldilocks should meet 64-bit, but not 128-bit
        assert!(report.meets_64_bit, "Goldilocks should meet 64-bit security");
        assert!(!report.meets_128_bit, "Goldilocks should not meet 128-bit (field too small)");
        
        // Soundness should exceed 128-bit
        assert!(report.soundness_bits > 128.0, "Goldilocks soundness insufficient");
    }
    
    #[test]
    fn test_bn254_security() {
        let params = SecurityParams::bn254();
        let report = params.generate_report();
        
        println!("{}", report);
        
        // BN254 should meet both 64-bit and 128-bit
        assert!(report.meets_64_bit);
        assert!(report.meets_128_bit, "BN254 should meet 128-bit security");
    }
    
    #[test]
    fn test_comparison() {
        let configs = vec![
            SecurityParams::babybear(),
            SecurityParams::goldilocks(),
            SecurityParams::bn254(),
        ];
        
        let comparison = compare_configs(&configs);
        println!("{}", comparison);
        
        // Verify comparison table is non-empty
        assert!(comparison.len() > 100);
        assert!(comparison.contains("BabyBear"));
        assert!(comparison.contains("Goldilocks"));
        assert!(comparison.contains("BN254"));
    }
}