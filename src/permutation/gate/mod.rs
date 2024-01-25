use std::sync::Arc;

use super::{DefaultSwapSchedule, SwapIndexOutOfRange};

mod witness_generator;
use plonky2::{
    field::extension::Extendable, hash::hash_types::RichField, iop::target::Target,
    plonk::circuit_builder::CircuitBuilder,
};
use witness_generator::PermutationGateWitnessGenerator;

/// In this module, we implement the `Gate` trait for `PermutationGate`
mod gate_implementation;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone)]
pub struct PermutationGate {
    // the number of items this gate acts on
    n_objects: usize,
    // the sequence of swaps this gate optionally executes
    swap_schedule: Arc<Vec<(usize, usize)>>,
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
            output_wires[idx1] = Self::compute_idx1_wire(n_objects, swap_schedule.len(), swap_nr);
            output_wires[idx2] = Self::compute_idx2_wire(n_objects, swap_schedule.len(), swap_nr);
        }

        Ok(Self {
            n_objects,
            swap_schedule: Arc::new(swap_schedule),
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
        Self::compute_selector_wire(self.n_objects, self.swap_schedule.len(), i)
    }

    /// The gate needs `2 * swap_schedule.len()` new wires to store the
    /// intermediate values. We have two helper functions to provide their positions.
    pub fn idx1_wire(&self, i: usize) -> usize {
        debug_assert!(i < self.swap_schedule.len());
        Self::compute_idx1_wire(self.n_objects, self.swap_schedule.len(), i)
    }

    /// The gate needs `2 * swap_schedule.len()` new wires to store the
    /// intermediate values. We have two helper functions to provide their positions.
    pub fn idx2_wire(&self, i: usize) -> usize {
        debug_assert!(i < self.swap_schedule.len());
        Self::compute_idx2_wire(self.n_objects, self.swap_schedule.len(), i)
    }

    /// Values in a circuit are immutable, so we have to read the output of
    /// the permutation on some wires that are different from the ones provided
    /// as input values.
    pub fn output_wire(&self, i: usize) -> usize {
        debug_assert!(i < self.n_objects);
        self.n_objects + i
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
    fn compute_selector_wire(n_objects: usize, _schedule_length: usize, i: usize) -> usize {
        2 * n_objects + i
    }
    fn compute_idx1_wire(n_objects: usize, schedule_length: usize, i: usize) -> usize {
        2 * n_objects + schedule_length + i
    }
    fn compute_idx2_wire(n_objects: usize, schedule_length: usize, i: usize) -> usize {
        2 * n_objects + 2 * schedule_length + i
    }
}

mod out_of_the_box_general_permutation_gates {
    use super::{super::SwapSchedule, PermutationGate};
    use std::{collections::BTreeMap, sync::Mutex};

    static CACHED_GATES: Mutex<BTreeMap<(usize, bool), PermutationGate>> =
        Mutex::new(BTreeMap::new());

    pub fn general_permutation_gate<S: SwapSchedule>(
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
            S::get_swap_sequence(n_objects),
            enforce_boolean_selectors,
        )
        .expect("the bubblesort schedule does not contain values greater or equal to n_objects");
        cache_lock.insert((n_objects, enforce_boolean_selectors), gate.clone());

        gate
    }
}
use out_of_the_box_general_permutation_gates::general_permutation_gate;

pub trait ApplyPermutation {
    /// enforces the `inputs` targets to be a permutation of the `outputs` targets.
    /// The permutation is determined by the values contained in `swap_selectors`.
    ///
    /// `swap_selectors` are supposed to be boolean values.
    /// If `enforce_boolean_selectors` is set to `true`, this constraint is forced
    /// automatically by the permutation gate, otherwise the user has to take care
    /// of this additional constraint elsewhere in the circuit.
    fn add_permutation_gate(
        &mut self,
        inputs: &[Target],
        swap_selectors: &[Target],
        outputs: &[Target],
        enforce_boolean_selectors: bool,
    ) -> Result<(), ()>;

    /// Get the number of swap selectors needed by the gate used in [ApplyPermutation::apply_permutation]
    fn permutation_swap_schedule_length(&self, n_objects: usize) -> usize;
}

impl<F: RichField + Extendable<D>, const D: usize> ApplyPermutation for CircuitBuilder<F, D> {
    fn add_permutation_gate(
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

        let gate = general_permutation_gate::<DefaultSwapSchedule>(
            inputs.len(),
            enforce_boolean_selectors,
        );
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

    fn permutation_swap_schedule_length(&self, n_objects: usize) -> usize {
        general_permutation_gate::<DefaultSwapSchedule>(n_objects, false)
            .swap_schedule()
            .len()
    }
}
