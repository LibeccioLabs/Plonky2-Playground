use super::{PermutationGate, PermutationGateWitnessGenerator};

use plonky2::{
    field::{extension::Extendable, types::Field},
    gates::gate::Gate,
    hash::hash_types::RichField,
    iop::{
        ext_target::ExtensionTarget,
        generator::{SimpleGenerator, WitnessGeneratorRef},
    },
    plonk::{circuit_builder::CircuitBuilder, circuit_data::CommonCircuitData},
    util::serialization::{Buffer, IoResult, Read, Write},
};

impl<F: RichField + Extendable<D>, const D: usize> Gate<F, D> for PermutationGate {
    // Seems standard practice to just output the debug info of the gate.
    fn id(&self) -> String {
        format!("{self:?}")
    }

    // We serialize self as if we were serializing any other struct, the
    // only difference being that we use plonky2 methods to do it.
    fn serialize(&self, dst: &mut Vec<u8>, _common_data: &CommonCircuitData<F, D>) -> IoResult<()> {
        dst.write_usize(self.n_objects)?;
        // Here we write each usize manually in order to avoid
        // having to transform `self.swap_schedule`, which is a
        // `Vec<(usize, usize)>` into a `Vec<usize>`.
        dst.write_usize(self.swap_schedule.len())?;
        for (idx1, idx2) in self.swap_schedule.iter() {
            dst.write_usize(*idx1)?;
            dst.write_usize(*idx2)?;
        }
        dst.write_bool(self.enforce_boolean_selectors)?;
        Ok(())
    }

    // We deserialize self as if we were serializing any other struct, the
    // only difference being that we use plonky2 methods to do it.
    fn deserialize(src: &mut Buffer, _common_data: &CommonCircuitData<F, D>) -> IoResult<Self>
    where
        Self: Sized,
    {
        let n_objects = src.read_usize()?;
        let swap_schedule_len = src.read_usize()?;
        let mut swap_schedule = Vec::with_capacity(swap_schedule_len);
        for _ in 0..swap_schedule_len {
            swap_schedule.push((src.read_usize()?, src.read_usize()?));
        }
        let enforce_boolean_selectors = src.read_bool()?;

        Self::try_new(n_objects, swap_schedule, enforce_boolean_selectors)
            .map_err(|_| plonky2::util::serialization::IoError)
    }

    // This gate will impose a number of coinstraints among targets, and this
    // function tells to the external world how many of them there are.
    //
    // This count does not include constraints that are indirectly implied by
    // the gate. More precisely, if this gate uses another gate `GateX`,
    // the constraints imposed by `GateX` must not be counted for this gate's
    // num_constraints.
    //
    // For example, this gate uses two or three constraints for every swap
    // to ensure that the swaps actually do what the "swap" word means.
    fn num_constraints(&self) -> usize {
        // we impose two or three constraints for each swap in the schedule.
        // An optional constraint to impose that the swap selector is boolean,
        // the others to enforce that the swapped items are a permutation of
        // the two inputs to the swap.
        (if self.enforce_boolean_selectors { 3 } else { 2 }) * self.swap_schedule.len()
    }

    fn degree(&self) -> usize {
        // Every mandatory constraint is of the form
        // `(x - (selector * y + (1 - selector) * z)) == 0`
        // which is a degree 2 constraint.
        //
        // while the optional constraint that forces `selector` to be boolean
        // is `selector * (selector - 1) == 0`, which is also of degree 2.
        2
    }

    fn num_constants(&self) -> usize {
        // This gate uses no constants
        0
    }

    // When evaluating our circuit, how many values do we need the knowledge of?
    // This function answers that question.
    fn num_wires(&self) -> usize {
        // We need `self.n_objects` values to input the objects we are about to act on
        self.n_objects +
        // plus, we need a boolean wire for every item in the swap schedule
        // plus, we need two intermediate values to make the swap,
        // for every item in the swap schedule
        3 * self.swap_schedule.len()
    }

    // Given some concrete values for the wires, computes the value for all the
    // constraints imposed by the gate. This function has no ZKP
    // functionalities, and can be used to check that everything works before
    // the prover starts producing the actual proof.
    fn eval_unfiltered(
        &self,
        vars: plonky2::plonk::vars::EvaluationVars<F, D>,
    ) -> Vec<<F as Extendable<D>>::Extension> {
        // For some reason, Rust does not allow me to make this a const value.
        #[allow(non_snake_case)] // This is supposed to be a constant.
        let ONE = <<F as Extendable<D>>::Extension as Field>::ONE;

        // Since variables in circuits are immutable, we use the items_tracker
        // to emulate the array struct. It will track which wires the items
        // end up in, and at the end it will contain the wire numbers
        // that correspond to the output of `self.output_wires`
        let mut items_tracker = Vec::with_capacity(self.n_objects);
        for i in 0..self.n_objects {
            items_tracker.push(self.input_wire(i));
        }

        let mut constraints = Vec::with_capacity(Gate::<F, D>::num_constraints(self));

        for (swap_nr, (idx1, idx2)) in self.swap_schedule_enum() {
            // We remind that selector will be a boolean value, so either 0 or 1.
            let selector = vars.local_wires[self.selector_wire(swap_nr)];
            // If `self.enforce_bool_selectors == false`, we delegate the check
            // that the selectors are indeed boolean to the broader circuit.
            // This makes this gate less safe, but allows for some optimizations.
            if self.enforce_boolean_selectors {
                constraints.push(selector * (selector - ONE));
            }

            let new_idx1_wire = self.idx1_wire(swap_nr);
            let new_idx2_wire = self.idx2_wire(swap_nr);
            // the value at `new_idx1_wire` is the `idx1`-th item,
            // if selector == 0, otherwise it is the `idx2`-th item.
            constraints.push(
                vars.local_wires[new_idx1_wire]
                    - (ONE - selector) * vars.local_wires[items_tracker[idx1]]
                    - selector * vars.local_wires[items_tracker[idx2]],
            );
            // analogous to the previous constraint.
            constraints.push(
                vars.local_wires[new_idx2_wire]
                    - selector * vars.local_wires[items_tracker[idx1]]
                    - (ONE - selector) * vars.local_wires[items_tracker[idx2]],
            );

            // We tell the items_tracker that now
            // the `idx1`-th and `idx2`-th items are located in
            // the `new_idx1_wire`-th and `new_idx2_wire`-th wires.
            items_tracker[idx1] = new_idx1_wire;
            items_tracker[idx2] = new_idx2_wire;
        }

        // as sanity check, we want items_tracker to be the same as self.output_wires
        debug_assert!(self
            .output_wires
            .iter()
            .zip(items_tracker)
            .all(|(a, b)| *a == b));

        constraints
    }

    // AFAIK, this tells the circuit builder what constraints have to be proved.
    // I was not able to prove it exactly, but I am pretty sure this is the
    // function we use to tell what a gate means in terms of plonk operations.
    fn eval_unfiltered_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: plonky2::plonk::vars::EvaluationTargets<D>,
    ) -> Vec<ExtensionTarget<D>> {
        let mut items_tracker = Vec::with_capacity(self.n_objects);
        for i in 0..self.n_objects {
            items_tracker.push(vars.local_wires[self.input_wire(i)]);
        }

        let mut constraints = Vec::with_capacity(Gate::<F, D>::num_constraints(self));

        for (swap_nr, (idx1, idx2)) in self.swap_schedule_enum() {
            let selector = vars.local_wires[self.selector_wire(swap_nr)];
            if self.enforce_boolean_selectors {
                let one = builder.one_extension();
                let not_selector = builder.sub_extension(one, selector);
                constraints.push(builder.mul_extension(selector, not_selector));
            }

            let new_idx1_wire = self.idx1_wire(swap_nr);
            let new_idx2_wire = self.idx2_wire(swap_nr);

            let new_idx1_target = vars.local_wires[new_idx1_wire];
            let new_idx2_target = vars.local_wires[new_idx2_wire];

            // We push the constraints that impose `(new_idx1_target, new_idx2_target)`
            // to be equal to (items_tracker[idx1], items_tracker[idx2]) or to its swap.

            let swap_out =
                builder.select_ext_generalized(selector, items_tracker[idx2], items_tracker[idx1]);
            constraints.push(builder.sub_extension(new_idx1_target, swap_out));

            let swap_out =
                builder.select_ext_generalized(selector, items_tracker[idx1], items_tracker[idx2]);
            constraints.push(builder.sub_extension(new_idx2_target, swap_out));

            // We update the items_tracker so that the next constraints (if any)
            // involving idx1 or idx2 will be imposed on `(new_idx1_target, new_idx2_target)`
            items_tracker[idx1] = new_idx1_target;
            items_tracker[idx2] = new_idx2_target;
        }

        constraints
    }

    // This returns a list of "devices" used to extrapolate witness values,
    // given the gate's input.
    //
    // Plonky2 assumes that the gate performs a distinct operation for every device in the list.
    // This means that, since in our case we want to perform only one permutation per row,
    // we have to return a vector containing a single generator.
    fn generators(&self, row: usize, _local_constants: &[F]) -> Vec<WitnessGeneratorRef<F, D>> {
        let swap_generator = PermutationGateWitnessGenerator {
            row,
            gate: self.clone(),
        }
        .adapter();

        vec![WitnessGeneratorRef::new(swap_generator)]
    }
}
