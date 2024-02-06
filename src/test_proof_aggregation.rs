use plonky2::{
    field::types::Field64,
    hash::poseidon::PoseidonHash,
    iop::witness::{PartialWitness, WitnessWrite},
    plonk::{
        circuit_builder::CircuitBuilder,
        circuit_data::{CircuitConfig, CircuitData},
        config::{GenericConfig, Hasher},
        proof::ProofWithPublicInputs,
    },
};

use crate::time_it;

// Degree of field extension in PlonkConfig.
const D: usize = 2;
type PGConfig = plonky2::plonk::config::PoseidonGoldilocksConfig;
type BaseField = <PGConfig as GenericConfig<D>>::F;

const N_VERIFY_REPETITIONS: usize = 100;
const INPUT_LENGTH: usize = 100;
const N_AGGREGATED_PROOFS: usize = 20;

/// This is a benchmark to showcase the verifier times of recursive proofs.
///
/// From the test results, one can see that the main factor that influences
/// the verification time is the number of public inputs of a circuit.
///
/// For this reason, in the second recursive circuit, we only expose a
/// commitment to the public inputs of the inner circuit. This way,
/// the proof verification can be kept fast, and anyone who is given
/// the public input values of the circuit can verify outside of the
/// zero knowledge circuit that they are equal to the actual
/// inputs of the inner circuit.
#[test]
fn test_verifying_time() {
    let circuit_config = CircuitConfig::standard_recursion_zk_config();

    let (base_circuit, random_proof_generator) =
        base_circuit(circuit_config.clone());

        let base_proofs: [_; N_AGGREGATED_PROOFS] =
        core::array::from_fn(|_| random_proof_generator(&base_circuit));
        
    println!(
        "proof size is: {}, and the number of public inputs is: {}",
        base_proofs[0].to_bytes().len(),
        base_proofs[0].public_inputs.len()
    );
    
    println!("Executing verify {} times", N_VERIFY_REPETITIONS);

    let mut proof_clones = Vec::from_iter((0..N_VERIFY_REPETITIONS).map(|_| base_proofs[0].clone()));
    time_it!(
        for _ in  0..N_VERIFY_REPETITIONS {
            base_circuit.verify(proof_clones.pop().unwrap()).expect("valid proof is rejected");
        };
        "verifying the proof takes {:?}"
    );

    // Now we build a recursive circuit that aggregates the proofs,
    // and that forwards all the public inputs of the inner circuits.
    // One can notice that verifying this circuit takes more time.

    let mut builder = CircuitBuilder::<BaseField, D>::new(circuit_config.clone());

    let verifier_data_target = builder.add_verifier_data_public_inputs();
    let proof_targets = Vec::from_iter(
        (0..N_AGGREGATED_PROOFS).map(|_| builder.add_virtual_proof_with_pis(&base_circuit.common)),
    );

    for t in proof_targets.iter() {
        builder.register_public_inputs(&t.public_inputs);
        builder.verify_proof::<PGConfig>(&t, &verifier_data_target, &base_circuit.common);
    }

    let mut witness = PartialWitness::new();
    for (t, p) in proof_targets.into_iter().zip(base_proofs.iter()) {
        witness.set_proof_with_pis_target(&t, p);

        for (t, v) in t.public_inputs.into_iter().zip(p.public_inputs.clone()) {
            witness.set_target(t, v);
        }
    }
    witness.set_verifier_data_target(&verifier_data_target, &base_circuit.verifier_only);

    let circuit = builder.build::<PGConfig>();

    let proof = 
        circuit.prove(witness).expect("proof generation fails");

    println!(
        "recursive proof size: {}, number of public inputs: {}",
        proof.to_bytes().len(),
        proof.public_inputs.len()
    );

    println!("Executing verify {} times", N_VERIFY_REPETITIONS);

    let mut proof_clones = Vec::from_iter((0..N_VERIFY_REPETITIONS).map(|_| proof.clone()));

    time_it!(
        for _ in 0..N_VERIFY_REPETITIONS {
            circuit.verify(proof_clones.pop().unwrap()).expect("valid proof is rejected");
        };
        "verifying the recursive proof takes {:?}"
    );

    // Finally, we build a circuit that does the same as the previous one,
    // but only exposes a commitment to the public inputs of the inner circuit.
    // We can observe that verification times are again comparable with those
    // of the non-recursive circuit.

    let mut builder = CircuitBuilder::<BaseField, D>::new(circuit_config.clone());

    let verifier_data_target = builder.add_verifier_data_public_inputs();
    let proof_target = builder.add_virtual_proof_with_pis(&circuit.common);
    let public_inputs_hash =
        builder.hash_n_to_hash_no_pad::<PoseidonHash>(proof_target.public_inputs.clone());

    builder.verify_proof::<PGConfig>(&proof_target, &verifier_data_target, &circuit.common);

    let mut witness = PartialWitness::new();
    witness.set_proof_with_pis_target(&proof_target, &proof);

    witness.set_hash_target(
        public_inputs_hash,
        plonky2::hash::hashing::hash_n_to_hash_no_pad::<
            BaseField,
            <PoseidonHash as Hasher<BaseField>>::Permutation,
        >(&proof.public_inputs),
    );

    witness.set_verifier_data_target(&verifier_data_target, &circuit.verifier_only);

    let circuit = builder.build::<PGConfig>();

    let proof =
        circuit.prove(witness).expect("proof generation fails");

    println!(
        "2nd recursive proof size: {}, number of public inputs: {}",
        proof.to_bytes().len(),
        proof.public_inputs.len()
    );

    println!("Executing verify {} times", N_VERIFY_REPETITIONS);

    let mut proof_clones = Vec::from_iter((0..N_VERIFY_REPETITIONS).map(|_| proof.clone()));

    time_it!(
        for _ in 0..N_VERIFY_REPETITIONS {
            circuit.verify(proof_clones.pop().unwrap()).expect("valid proof is rejected");
        };
        "verifying the 2nd recursive proof takes {:?}"
    );
}

/// We don't care what the circuit actually does. We just know that this
/// function outputs a non-trivial circuit, together with a function
/// that can generate valid proofs for said circuit.
fn base_circuit(
    circuit_config: CircuitConfig,
) -> (
    CircuitData<BaseField, PGConfig, D>,
    impl Fn(&CircuitData<BaseField, PGConfig, D>) -> ProofWithPublicInputs<BaseField, PGConfig, D>,
) {
    let mut builder = CircuitBuilder::<BaseField, D>::new(circuit_config);
    let input: [_; INPUT_LENGTH] = builder.add_virtual_hashes(INPUT_LENGTH).try_into().unwrap();
    let hash_out_targets: [_; INPUT_LENGTH] = core::array::from_fn(|idx| {
        builder.hash_n_to_hash_no_pad::<PoseidonHash>(input[idx].elements.into())
    });
    for h in hash_out_targets {
        builder.register_public_inputs(&h.elements);
    }

    let circuit = builder.build::<PGConfig>();

    let random_proof_generator =
        move |circuit: &CircuitData<BaseField, PGConfig, D>|
            -> ProofWithPublicInputs<BaseField, PGConfig, D> {
        let mut witness = PartialWitness::new();
        for h in input {
            let input_value: [_; plonky2::hash::hash_types::NUM_HASH_OUT_ELTS] =
                core::array::from_fn(|_| BaseField::from_canonical_i64(rand::random()));
            witness.set_target_arr(&h.elements, &input_value);
        }

            circuit.prove(witness).expect("proof generation fails")
        
    };

    (circuit, random_proof_generator)
}
