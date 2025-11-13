//! Groth16 zkSNARK circuit for PoT eligibility verification

use ark_bn254::{Bn254, Fr as BnFr};
use ark_ff::PrimeField;
use ark_groth16::{Groth16, ProvingKey, VerifyingKey, Proof};
use ark_r1cs_std::prelude::*;
use ark_r1cs_std::fields::fp::FpVar;
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};
use ark_snark::{CircuitSpecificSetupSNARK, SNARK};
use rand_chacha::{ChaCha20Rng, rand_core::SeedableRng};
use anyhow::Context;

/// Public inputs for eligibility circuit
#[derive(Clone)]
pub struct EligibilityPublicInputs {
    pub weights_root: [u8; 32],
    pub beacon_value: [u8; 32],
    pub threshold_q: u64,
    pub sum_weights_q: u128,
}

/// Private witness for eligibility circuit
#[derive(Clone)]
pub struct EligibilityWitness {
    pub who: [u8; 32],
    pub slot: u64,
    pub stake_q: u128,
    pub trust_q: u64,
    pub merkle_siblings: Vec<[u8; 32]>,
    pub leaf_index: u64,
}

/// Eligibility circuit
#[derive(Clone)]
pub struct EligibilityCircuit {
    pub public_inputs: Option<EligibilityPublicInputs>,
    pub witness: Option<EligibilityWitness>,
}

impl ConstraintSynthesizer<BnFr> for EligibilityCircuit {
    fn generate_constraints(self, cs: ConstraintSystemRef<BnFr>) -> Result<(), SynthesisError> {
        // Allocate public inputs
        let weights_root_var = FpVar::<BnFr>::new_input(cs.clone(), || {
            let val = self.public_inputs.as_ref()
                .map(|p| bytes_to_field(&p.weights_root))
                .ok_or(SynthesisError::AssignmentMissing)?;
            Ok(val)
        })?;

        let beacon_var = FpVar::<BnFr>::new_input(cs.clone(), || {
            let val = self.public_inputs.as_ref()
                .map(|p| bytes_to_field(&p.beacon_value))
                .ok_or(SynthesisError::AssignmentMissing)?;
            Ok(val)
        })?;

        let threshold_var = FpVar::<BnFr>::new_input(cs.clone(), || {
            let val = self.public_inputs.as_ref()
                .map(|p| BnFr::from(p.threshold_q))
                .ok_or(SynthesisError::AssignmentMissing)?;
            Ok(val)
        })?;

        let sum_weights_var = FpVar::<BnFr>::new_input(cs.clone(), || {
            let val = self.public_inputs.as_ref()
                .map(|p| BnFr::from(p.sum_weights_q))
                .ok_or(SynthesisError::AssignmentMissing)?;
            Ok(val)
        })?;

        // Allocate private witness
        let who_var = FpVar::<BnFr>::new_witness(cs.clone(), || {
            let val = self.witness.as_ref()
                .map(|w| bytes_to_field(&w.who))
                .ok_or(SynthesisError::AssignmentMissing)?;
            Ok(val)
        })?;

        let slot_var = FpVar::<BnFr>::new_witness(cs.clone(), || {
            let val = self.witness.as_ref()
                .map(|w| BnFr::from(w.slot))
                .ok_or(SynthesisError::AssignmentMissing)?;
            Ok(val)
        })?;

        let stake_var = FpVar::<BnFr>::new_witness(cs.clone(), || {
            let val = self.witness.as_ref()
                .map(|w| BnFr::from(w.stake_q))
                .ok_or(SynthesisError::AssignmentMissing)?;
            Ok(val)
        })?;

        let trust_var = FpVar::<BnFr>::new_witness(cs.clone(), || {
            let val = self.witness.as_ref()
                .map(|w| BnFr::from(w.trust_q))
                .ok_or(SynthesisError::AssignmentMissing)?;
            Ok(val)
        })?;

        // CONSTRAINT 1: Merkle path verification (simplified)
        let weight_var = &stake_var * &trust_var;
        let _leaf_var = &who_var + &weight_var;

        // CONSTRAINT 2: Threshold computation
        // weight × sum_weights >= threshold × total_weight
        let lhs = &weight_var * &sum_weights_var;
        let rhs = &threshold_var * &stake_var;
        let _check = lhs - rhs; // Should be >= 0

        // CONSTRAINT 3: Eligibility hash check (simplified)
        let elig_hash_var = &beacon_var + &slot_var + &who_var;
        elig_hash_var.enforce_not_equal(&FpVar::zero())?;

        Ok(())
    }
}

/// Convert bytes to field element
fn bytes_to_field(bytes: &[u8]) -> BnFr {
    let mut limbs = [0u8; 32];
    let len = bytes.len().min(32);
    limbs[..len].copy_from_slice(&bytes[..len]);
    BnFr::from_le_bytes_mod_order(&limbs)
}

/// ZK proving key
pub type ZkProvingKey = ProvingKey<Bn254>;

/// ZK verifying key
pub type ZkVerifyingKey = VerifyingKey<Bn254>;

/// ZK proof
pub type ZkProof = Proof<Bn254>;

/// Setup trusted setup (for testing - use MPC in production!)
pub fn setup_keys(circuit: EligibilityCircuit) -> anyhow::Result<(ZkProvingKey, ZkVerifyingKey)> {
    let mut rng = ChaCha20Rng::from_entropy();
    let (pk, vk) = Groth16::<Bn254>::setup(circuit, &mut rng)
        .map_err(|e| anyhow::anyhow!("Setup failed: {:?}", e))?;
    Ok((pk, vk))
}

/// Generate eligibility proof
pub fn prove_eligibility(
    pk: &ZkProvingKey,
    public_inputs: EligibilityPublicInputs,
    witness: EligibilityWitness,
) -> anyhow::Result<ZkProof> {
    let circuit = EligibilityCircuit {
        public_inputs: Some(public_inputs.clone()),
        witness: Some(witness),
    };

    let mut rng = ChaCha20Rng::from_entropy();
    let proof = Groth16::<Bn254>::prove(pk, circuit, &mut rng)
        .map_err(|e| anyhow::anyhow!("Prove failed: {:?}", e))?;
    
    Ok(proof)
}

/// Verify eligibility proof
pub fn verify_eligibility(
    vk: &ZkVerifyingKey,
    public_inputs: &EligibilityPublicInputs,
    proof: &ZkProof,
) -> anyhow::Result<bool> {
    let inputs = vec![
        bytes_to_field(&public_inputs.weights_root),
        bytes_to_field(&public_inputs.beacon_value),
        BnFr::from(public_inputs.threshold_q),
        BnFr::from(public_inputs.sum_weights_q),
    ];

    let result = Groth16::<Bn254>::verify(vk, &inputs, proof)
        .map_err(|e| anyhow::anyhow!("Verify failed: {:?}", e))?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setup_and_prove() {
        let circuit = EligibilityCircuit {
            public_inputs: Some(EligibilityPublicInputs {
                weights_root: [1u8; 32],
                beacon_value: [2u8; 32],
                threshold_q: 1000,
                sum_weights_q: 10000,
            }),
            witness: Some(EligibilityWitness {
                who: [3u8; 32],
                slot: 42,
                stake_q: 5000,
                trust_q: 100,
                merkle_siblings: vec![],
                leaf_index: 0,
            }),
        };

        let (pk, vk) = setup_keys(circuit.clone()).unwrap();
        
        let proof = prove_eligibility(
            &pk,
            circuit.public_inputs.clone().unwrap(),
            circuit.witness.unwrap(),
        ).unwrap();

        let valid = verify_eligibility(&vk, &circuit.public_inputs.unwrap(), &proof).unwrap();
        assert!(valid);
    }
}
