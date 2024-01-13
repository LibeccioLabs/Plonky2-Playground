use std::sync::Arc;

use super::SwapIndexOutOfRange;

mod witness_generator;
use plonky2::{
    field::extension::Extendable, hash::hash_types::RichField, iop::target::Target,
    plonk::circuit_builder::CircuitBuilder,
};
use witness_generator::PermutationGateWitnessGenerator;

/// In this module, we implement the `Gate` trait for `PermutationGate`
mod gate_implementation;

#[derive(Debug, Clone)]
pub struct PermutationGate {
    // the number of items this gate acts on
    n_objects: usize,
    // the sequence of swaps this gate optionally executes
    swap_schedule: Arc<Vec<(usize, usize)>>,
    // where to find the output values.
    // It is derived from n_objects and swap_schedule.
    output_wires: Arc<Vec<usize>>,
    // If true, this gate will introduce a new constraint
    // for every swap, to enforce that the swap selectors are indeed boolean.
    enforce_boolean_selectors: bool,
}

// Utility functions to have a standardized, semantically meaningful way
// to access this gate's wires when evaluating it.
//
// Also, initialization methods.
impl PermutationGate {
    /// Given a swap schedule, tries initializing a `PermutationGate instance`.
    ///
    /// Fails if one of the usize values contained in
    /// `swap_schedule` is not smaller than `n_objects`.
    pub fn try_new(
        n_objects: usize,
        swap_schedule: Vec<(usize, usize)>,
        enforce_boolean_selectors: bool,
    ) -> Result<Self, SwapIndexOutOfRange> {
        let mut output_wires = Vec::with_capacity(n_objects);

        for i in 0..n_objects {
            output_wires.push(Self::compute_input_wire(i));
        }

        for (swap_nr, (idx1, idx2)) in swap_schedule.iter().map(|e| *e).enumerate() {
            if n_objects <= idx1 || n_objects <= idx2 {
                return Err(SwapIndexOutOfRange {
                    max_allowed: n_objects,
                    found: (None, idx1, idx2),
                });
            }
            // After the `swap_nr`-th swap, those wires contain the result
            // of the conditional swap of the `idx1`-th and `idx2`-th items.
            output_wires[idx1] = Self::compute_idx1_wire(n_objects, swap_nr);
            output_wires[idx2] = Self::compute_idx2_wire(n_objects, swap_nr);
        }

        Ok(Self {
            n_objects,
            swap_schedule: Arc::new(swap_schedule),
            output_wires: Arc::new(output_wires),
            enforce_boolean_selectors,
        })
    }

    /// Getter for the number of items being swapped.
    pub fn n_objects(&self) -> usize {
        self.n_objects
    }

    /// Getter for the sequence of swaps this gate performs.
    pub fn swap_schedule(&self) -> Arc<Vec<(usize, usize)>> {
        self.swap_schedule.clone()
    }

    /// The gate permutes `n_objects` inputs. This function outputs
    /// the `i`-th input wire's position.
    pub fn input_wire(&self, i: usize) -> usize {
        debug_assert!(i < self.n_objects);
        Self::compute_input_wire(i)
    }

    /// The gate needs `swap_schedule.len()` boolean selectors to determine how
    /// the items will be swapped around. This function outputs the `i`-th
    /// selector's position.
    pub fn selector_wire(&self, i: usize) -> usize {
        debug_assert!(i < self.swap_schedule.len());
        Self::compute_selector_wire(self.n_objects, i)
    }

    /// The gate needs `2 * swap_schedule.len()` new wires to store the
    /// intermediate values. We have two helper functions to provide their positions.
    pub fn idx1_wire(&self, i: usize) -> usize {
        debug_assert!(i < self.swap_schedule.len());
        Self::compute_idx1_wire(self.n_objects, i)
    }

    /// The gate needs `2 * swap_schedule.len()` new wires to store the
    /// intermediate values. We have two helper functions to provide their positions.
    pub fn idx2_wire(&self, i: usize) -> usize {
        debug_assert!(i < self.swap_schedule.len());
        Self::compute_idx2_wire(self.n_objects, i)
    }

    /// Values in a circuit are immutable, so we have to read the output of
    /// the permutation on some wires that are different from the ones provided
    /// as input values.
    pub fn output_wire(&self, i: usize) -> usize {
        debug_assert!(i < self.n_objects);
        self.output_wires[i]
    }

    /// Helper function to get an iterator over the swap schedule data.
    fn swap_schedule_enum<'a>(&'a self) -> impl Iterator<Item = (usize, (usize, usize))> + 'a {
        self.swap_schedule.iter().map(|e| *e).enumerate()
    }
}

// These helper functions are needed when we don't have a gate already,
// and we are in the process of building one, in `PermutationGate::try_new`
impl PermutationGate {
    fn compute_input_wire(i: usize) -> usize {
        i
    }
    fn compute_selector_wire(n_objects: usize, i: usize) -> usize {
        n_objects + 3 * i
    }
    fn compute_idx1_wire(n_objects: usize, i: usize) -> usize {
        n_objects + 3 * i + 1
    }
    fn compute_idx2_wire(n_objects: usize, i: usize) -> usize {
        n_objects + 3 * i + 2
    }
}

mod out_of_the_box_general_permutation_gates {
    use super::{super::bubblesort_schedule, PermutationGate};
    use std::{collections::BTreeMap, sync::Mutex};

    static CACHED_GATES: Mutex<BTreeMap<(usize, bool), PermutationGate>> =
        Mutex::new(BTreeMap::new());

    pub fn general_permutation_gate(
        n_objects: usize,
        enforce_boolean_selectors: bool,
    ) -> PermutationGate {
        let mut cache_lock = CACHED_GATES.lock().expect("Mutex is poisoned, aborting.");
        // If we already computed the gate with the same number of inputs
        // (which is very likely), it returns the cached result.
        if let Some(gate) = cache_lock.get(&(n_objects, enforce_boolean_selectors)) {
            return gate.clone();
        }

        // Otherwise, it computes the result, adds it to the cache, and returns it.
        let gate = PermutationGate::try_new(
            n_objects,
            bubblesort_schedule(n_objects),
            enforce_boolean_selectors,
        )
        .expect("the bubblesort schedule does not contain values greater or equal to n_objects");
        cache_lock.insert((n_objects, enforce_boolean_selectors), gate.clone());

        gate
    }
}
use out_of_the_box_general_permutation_gates::general_permutation_gate;

pub trait ApplyPermutation {
    fn apply_permutation(
        &mut self,
        inputs: &[Target],
        swap_selectors: &[Target],
        outputs: &[Target],
        enforce_boolean_selectors: bool,
    ) -> Result<(), ()>;
}

impl<F: RichField + Extendable<D>, const D: usize> ApplyPermutation for CircuitBuilder<F, D> {
    fn apply_permutation(
        &mut self,
        inputs: &[Target],
        swap_selectors: &[Target],
        outputs: &[Target],
        enforce_boolean_selectors: bool,
    ) -> Result<(), ()> {
        // We need the same number of input and output items.
        if inputs.len() != outputs.len() {
            return Err(());
        }

        let gate = general_permutation_gate(inputs.len(), enforce_boolean_selectors);
        // We need exactly one selector for every swap.
        if swap_selectors.len() != gate.swap_schedule.len() {
            return Err(());
        }

        let (gate_row, op) = self.find_slot(gate.clone(), &[], &[]);

        // this gate does not allow more than one operation per row at the moment.
        if op != 0 {
            return Err(());
        }

        for (idx, input) in inputs.into_iter().enumerate() {
            self.connect(*input, Target::wire(gate_row, gate.input_wire(idx)));
        }

        for (swap_nr, selector) in swap_selectors.into_iter().enumerate() {
            self.connect(
                *selector,
                Target::wire(gate_row, gate.selector_wire(swap_nr)),
            );
        }

        for (idx, output) in outputs.into_iter().enumerate() {
            self.connect(*output, Target::wire(gate_row, gate.output_wire(idx)));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use plonky2::{
        field::types::Field64,
        iop::witness::{PartialWitness, WitnessWrite},
        plonk::{circuit_builder::CircuitBuilder, config::GenericConfig},
    };

    use crate::permutation::swap_schedule::from_permutation_to_bubble_sort_swap_schedule;

    use super::{
        super::{inverse_permutation, PermutationsIter},
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

        let start_timer = std::time::Instant::now();

        let circuit_config =
            plonky2::plonk::circuit_data::CircuitConfig::standard_recursion_zk_config();

        let items: [BaseField; N_OBJECTS] =
            core::array::from_fn(|i| BaseField::from_canonical_i64(i as i64));

        let p_gate = general_permutation_gate(N_OBJECTS, ENFORCE_BOOL_SELECTORS);
        let n_swap_selectors = p_gate.swap_schedule().len();

        let mut builder = CircuitBuilder::<BaseField, D>::new(circuit_config.clone());

        let virtual_pub_inputs = builder.add_virtual_public_input_arr::<N_OBJECTS>();
        let virtual_pub_inputs_permutation = builder.add_virtual_public_input_arr::<N_OBJECTS>();

        let virtual_swap_selectors = builder.add_virtual_targets(n_swap_selectors);

        builder
            .apply_permutation(
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

        let end_timer = std::time::Instant::now();

        println!("Building the circuit takes {:?}", end_timer - start_timer);

        // will be properly initialized and mutated by PermutationsIter
        let mut permutation_buffer = [0; N_OBJECTS];

        // will be properly initialized and mutated to store the inverse of
        // the permutation stored in permutation_buffer.
        let mut inverse_p = [0; N_OBJECTS];

        // We will save the individual proofs, and aggregate them later with another circuit.
        let mut proofs = vec![];
        for permutation in PermutationsIter::from(permutation_buffer.as_mut_slice()) {
            let mut permutation = permutation.expect("The iterator yields a mutable reference to the slice given to the constructor. Iteration fails only if we try getting more than one such ref at the same time.");
            println!("{permutation:?}");

            let start_timer = std::time::Instant::now();

            let mut witness = PartialWitness::<BaseField>::new();

            // The input is always the 0..N_OBJECTS range.
            witness.set_target_arr(virtual_pub_inputs.as_slice(), items.as_slice());

            // if we apply the permutation P to the input items,
            // calling Q its inverse we will observe an output consisting of
            // (0..N_OBJECTS).map(|idx| Q(idx))
            inverse_permutation(*permutation, inverse_p.as_mut_slice());
            let permutated_items: [_; N_OBJECTS] =
                core::array::from_fn(|idx| items[inverse_p[idx]]);
            witness.set_target_arr(
                virtual_pub_inputs_permutation.as_slice(),
                permutated_items.as_slice(),
            );

            let selectors: Vec<BaseField> =
                from_permutation_to_bubble_sort_swap_schedule(*permutation)
                    .into_iter()
                    .map(|(selector, _idx1, _idx2)| BaseField::from_canonical_i64(selector.into()))
                    .collect();
            witness.set_target_arr(virtual_swap_selectors.as_slice(), selectors.as_slice());

            proofs.push(p_circuit.prove(witness).expect("proof generation fails."));

            let end_timer = std::time::Instant::now();

            println!("Computing the proof takes {:?}", end_timer - start_timer);

            let proof = proofs.last().expect("we just pushed to this vector.");
            println!("proof size: {}", proof.to_bytes().len());

            let start_timer = std::time::Instant::now();

            p_circuit
                .verify(proof.clone())
                .expect("proof verification fails.");

            let end_timer = std::time::Instant::now();

            println!("Verifying the proof takes {:?}", end_timer - start_timer);
        }

        let start_timer = std::time::Instant::now();

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

        let end_timer = std::time::Instant::now();

        println!(
            "Building the recursive circuit takes {:?}",
            end_timer - start_timer
        );

        let start_timer = std::time::Instant::now();

        let aggregated_proof = aggregated_proof_circuit
            .prove(witness)
            .expect("Generation of aggregated proof fails.");

        let end_timer = std::time::Instant::now();

        println!(
            "Computing the recursive proof takes {:?}",
            end_timer - start_timer
        );

        // We can observe that combining all the previous proofs takes a long time,
        // but the size of the combined proof is about the same as any of the
        // mon-recursive proofs!
        println!(
            "Size of recursive proof: {}",
            aggregated_proof.to_bytes().len()
        );

        let start_timer = std::time::Instant::now();

        aggregated_proof_circuit
            .verify(aggregated_proof)
            .expect("Verification of aggregated proof fails.");

        let end_timer = std::time::Instant::now();

        println!(
            "Verifying the recursive proof takes {:?}",
            end_timer - start_timer
        );
    }
}
