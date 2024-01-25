use plonky2::{
    field::extension::Extendable,
    gates::gate::Gate,
    hash::hash_types::RichField,
    iop::{
        generator::SimpleGenerator,
        target::Target,
        witness::{Witness, WitnessWrite},
    },
    plonk::circuit_data::CommonCircuitData,
    util::serialization::{Buffer, IoResult, Read, Write},
};

use super::PermutationGate;

#[derive(Debug)]
pub struct PermutationGateWitnessGenerator {
    pub row: usize,
    pub gate: PermutationGate,
}

impl PermutationGateWitnessGenerator {
    fn target(&self, column: usize) -> Target {
        Target::wire(self.row, column)
    }
}

impl<F: RichField + Extendable<D>, const D: usize> SimpleGenerator<F, D>
    for PermutationGateWitnessGenerator
{
    fn id(&self) -> String {
        format! {"self:?"}
    }

    fn serialize(&self, dst: &mut Vec<u8>, common_data: &CommonCircuitData<F, D>) -> IoResult<()> {
        dst.write_usize(self.row)?;
        self.gate.serialize(dst, common_data)
    }

    fn deserialize(src: &mut Buffer, common_data: &CommonCircuitData<F, D>) -> IoResult<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            row: src.read_usize()?,
            gate: PermutationGate::deserialize(src, common_data)?,
        })
    }
    fn dependencies(&self) -> Vec<Target> {
        let gate = &self.gate;
        // We need to observe one target for every input item,
        // plus one target for each swap in the schedule.
        let mut dependencies = Vec::with_capacity(gate.n_objects + gate.swap_schedule.len());

        dependencies.extend((0..gate.n_objects).map(|idx| self.target(gate.input_wire(idx))));
        dependencies.extend(
            (0..gate.swap_schedule.len()).map(|swap_nr| self.target(gate.selector_wire(swap_nr))),
        );

        dependencies
    }

    fn run_once(
        &self,
        witness: &plonky2::iop::witness::PartitionWitness<F>,
        out_buffer: &mut plonky2::iop::generator::GeneratedValues<F>,
    ) {
        let gate = &self.gate;

        // A vector of Target that keeps track of the positions of the items the
        // next swap will act on. After each swap, this list will be updated to
        // represent the swapped items'new positions.
        let mut items_tracker = Vec::with_capacity(gate.n_objects);
        items_tracker.extend((0..gate.n_objects).map(|idx| self.target(gate.input_wire(idx))));

        // A vector of field elements that keeps track of the values of the items
        // the permutation is acting on. similarly to `items_tracker`, it is
        // updated after each swap.
        let mut item_values = Vec::with_capacity(gate.n_objects);
        item_values.extend(
            items_tracker
                .iter()
                .map(|target| witness.get_target(*target)),
        );

        for (swap_nr, (idx1, idx2)) in gate.swap_schedule_enum() {
            let selector = witness.get_target(self.target(gate.selector_wire(swap_nr)));
            let new_item1_target = self.target(gate.idx1_wire(swap_nr));
            let new_item2_target = self.target(gate.idx2_wire(swap_nr));

            out_buffer.set_target(
                new_item1_target,
                item_values[idx1] + selector * (item_values[idx2] - item_values[idx1]),
            );
            out_buffer.set_target(
                new_item2_target,
                item_values[idx2] + selector * (item_values[idx1] - item_values[idx2]),
            );

            items_tracker[idx1] = new_item1_target;
            items_tracker[idx2] = new_item2_target;

            if selector != F::ZERO {
                item_values.swap(idx1, idx2);
            }
        }

        for (idx, value) in item_values.into_iter().enumerate() {
            out_buffer.set_target(self.target(gate.output_wire(idx)), value);
        }
    }
}
