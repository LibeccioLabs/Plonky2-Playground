use plonky2::{
    field::types::Field64,
    iop::witness::{PartialWitness, WitnessWrite},
    plonk::{circuit_builder::CircuitBuilder, config::GenericConfig},
};

use super::{
    super::{inverse_permutation, DefaultSwapSchedule, PermutationsIter, SwapSchedule},
    general_permutation_gate, ApplyPermutation,
};

/// Tests the correct computation of the proof of every possible permutation
/// of 4 items.
///
/// Then, it computes the aggregated proof using a recursive circuit.
///
/// WARNING: this test takes about 2 minutes on a lenovo yoga with `--release`.
/// For the love of god, please do not run it on debug mode,
/// it is about 30 times slower.
#[test]
fn test_gate_4_objects() {
    const N_OBJECTS: usize = 4;

    // The permutation gate gives the option to check that the selectors
    // used to swap the items around are boolean values inside, or outside
    // the gate itself. Checking outside of the gate reduces the number of
    // constraints imposed by the gate, but requires the circuit builder to
    // make the check manually elsewhere.
    const ENFORCE_BOOL_SELECTORS: bool = false;

    // Degree of field extension in PlonkConfig.
    // What is this for? IDK.
    const D: usize = 2;
    type PlonkConfig = plonky2::plonk::config::PoseidonGoldilocksConfig;
    type BaseField = <PlonkConfig as GenericConfig<D>>::F;

    let circuit_config =
        plonky2::plonk::circuit_data::CircuitConfig::standard_recursion_zk_config();

    let items: [BaseField; N_OBJECTS] =
        core::array::from_fn(|i| BaseField::from_canonical_i64(i as i64));

    let p_gate = general_permutation_gate::<DefaultSwapSchedule>(N_OBJECTS, ENFORCE_BOOL_SELECTORS);
    let n_swap_selectors = p_gate.swap_schedule().len();

    let mut builder = CircuitBuilder::<BaseField, D>::new(circuit_config.clone());

    let virtual_pub_inputs = builder.add_virtual_public_input_arr::<N_OBJECTS>();
    let virtual_pub_inputs_permutation = builder.add_virtual_public_input_arr::<N_OBJECTS>();

    let virtual_swap_selectors = builder.add_virtual_targets(n_swap_selectors);

    builder
        .add_permutation_gate(
            virtual_pub_inputs.as_slice(),
            virtual_swap_selectors.as_slice(),
            virtual_pub_inputs_permutation.as_slice(),
            ENFORCE_BOOL_SELECTORS,
        )
        .expect("Circuit building fails while adding the permutation gate.");

    // If we don't enforce the targets to be boolean inside the gate,
    // we must do it outside.
    if !ENFORCE_BOOL_SELECTORS {
        for selector in virtual_swap_selectors.iter() {
            let bool_virtual_target = builder.add_virtual_bool_target_safe();
            builder.connect(bool_virtual_target.target, *selector);
        }
    }

    let p_circuit = builder.build::<PlonkConfig>();

    // will be properly initialized and mutated by PermutationsIter
    let mut permutation_buffer = [0; N_OBJECTS];

    // will be properly initialized and mutated to store the inverse of
    // the permutation stored in permutation_buffer.
    let mut inverse_p = [0; N_OBJECTS];

    // We will save the individual proofs, and aggregate them later with another circuit.
    let mut proofs = vec![];
    for permutation in PermutationsIter::from(permutation_buffer.as_mut_slice()) {
        let mut permutation = permutation.expect("The iterator yields a mutable reference to the slice given to the constructor. Iteration fails only if we try getting more than one such ref at the same time.");

        crate::time_it! {{
                let mut witness = PartialWitness::<BaseField>::new();

                // The input is always the 0..N_OBJECTS range.
                witness.set_target_arr(virtual_pub_inputs.as_slice(), items.as_slice());

                // if we apply the permutation P to the input items,
                // calling Q its inverse we will observe an output consisting of
                // (0..N_OBJECTS).map(|idx| Q(idx))
                inverse_permutation(*permutation, inverse_p.as_mut_slice());
                let permutated_items: [_; N_OBJECTS] = core::array::from_fn(|idx| items[inverse_p[idx]]);
                witness.set_target_arr(
                    virtual_pub_inputs_permutation.as_slice(),
                    permutated_items.as_slice(),
                );

                let selectors: Vec<BaseField> =
                    DefaultSwapSchedule::permutation_to_swap_schedule(*permutation)
                        .into_iter()
                        .map(|(selector, _idx1, _idx2)| BaseField::from_canonical_i64(selector.into()))
                        .collect();
                witness.set_target_arr(virtual_swap_selectors.as_slice(), selectors.as_slice());

                proofs.push(p_circuit.prove(witness).expect("proof generation fails."));
            }; "Computing the proof takes {:?}"
        };

        let proof = proofs.last().expect("we just pushed to this vector.");
        println!("proof size: {}", proof.to_bytes().len());

        crate::time_it! {
            p_circuit
                .verify(proof.clone())
                .expect("proof verification fails.");
            "Verifying the proof takes {:?}"
        };
    }

    // now we build a circuit that aggregates all the previous proofs into one.

    let mut builder = CircuitBuilder::<BaseField, D>::new(circuit_config);

    // we want to aggregate `factorial(N_OBJECTS)` proofs.
    // It is easier in this particular instance to build the circuit and
    // the witness at the same time.
    let mut witness = PartialWitness::<BaseField>::new();

    for proof in proofs.iter() {
        // We have to gather some structural info about the circuit that
        // generated the individual proofs before, and we use this info to
        // prepare some targets in the circuit we are building now.
        let proof_with_pis = builder.add_virtual_proof_with_pis(&p_circuit.common);

        let inner_verifier_data =
            builder.add_virtual_verifier_data(p_circuit.common.config.fri_config.cap_height);

        // here we actually wire the circuit to prove that another proof is correct.
        builder.verify_proof::<PlonkConfig>(
            &proof_with_pis,
            &inner_verifier_data,
            &p_circuit.common,
        );

        // here we set the piece of the witness that corresponds to the proof we just
        // instructed our recursive circuit to prove.
        witness.set_proof_with_pis_target(&proof_with_pis, proof);
        witness.set_verifier_data_target(&inner_verifier_data, &p_circuit.verifier_only);
    }

    let aggregated_proof_circuit = builder.build::<PlonkConfig>();

    let aggregated_proof = crate::time_it! {
        aggregated_proof_circuit
            .prove(witness)
            .expect("Generation of aggregated proof fails.");
        "Computing the recursive proof takes {:?}"
    };

    // We can observe that combining all the previous proofs takes a long time,
    // but the size of the combined proof is about the same as any of the
    // mon-recursive proofs!
    println!(
        "Size of recursive proof: {}",
        aggregated_proof.to_bytes().len()
    );

    crate::time_it! {
        aggregated_proof_circuit
            .verify(aggregated_proof)
            .expect("Verification of aggregated proof fails.");
        "Verifying the recursive proof takes {:?}"
    };
}
