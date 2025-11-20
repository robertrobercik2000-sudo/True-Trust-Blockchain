//! Hybrid PQC Commitment
//! 
//! Combines Falcon-512 and ML-KEM-768 public keys into a single fingerprint

use sha3::{Digest, Sha3_256};

/// Generate a fingerprint from Falcon-512 and ML-KEM-768 public keys
/// 
/// This creates a commitment to both public keys for hybrid security
pub fn pqc_fingerprint(falcon_pk: &[u8], mlkem_pk: &[u8]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(b"TT-HYBRID-PQC-v1");
    hasher.update(&(falcon_pk.len() as u64).to_le_bytes());
    hasher.update(falcon_pk);
    hasher.update(&(mlkem_pk.len() as u64).to_le_bytes());
    hasher.update(mlkem_pk);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pqc_fingerprint_deterministic() {
        let falcon = vec![1u8; 897];
        let mlkem = vec![2u8; 1184];
        
        let fp1 = pqc_fingerprint(&falcon, &mlkem);
        let fp2 = pqc_fingerprint(&falcon, &mlkem);
        
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn test_pqc_fingerprint_different() {
        let falcon1 = vec![1u8; 897];
        let falcon2 = vec![2u8; 897];
        let mlkem = vec![3u8; 1184];
        
        let fp1 = pqc_fingerprint(&falcon1, &mlkem);
        let fp2 = pqc_fingerprint(&falcon2, &mlkem);
        
        assert_ne!(fp1, fp2);
    }
}
