//! PoZS (Proof-of-ZK-Shares) zkSNARK layer - Groth16/BN254

#[cfg(feature = "zk-proofs")]
pub mod groth16;

#[cfg(feature = "zk-proofs")]
pub mod keccak_gadget;

#[cfg(feature = "zk-proofs")]
pub use groth16::{
    EligibilityCircuit, EligibilityPublicInputs, EligibilityWitness,
    setup_keys, prove_eligibility, verify_eligibility,
    ZkProvingKey, ZkVerifyingKey, ZkProof,
};

// Stub for when zk-proofs feature is disabled
#[cfg(not(feature = "zk-proofs"))]
pub fn zk_proofs_disabled() {
    compile_error!("ZK proofs feature is not enabled");
}
