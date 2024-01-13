// NOTES:
// The documentation of `plonky2::iop::wire::Wire` is very short, but
// i think crucially useful.

mod permutation;
pub use permutation::{
    bubblesort_schedule, from_permutation_to_bubble_sort_swap_schedule, inverse_permutation,
    ApplyPermutation, KnuthL, PermutationGate, PermutationsIter, SwapIndexOutOfRange,
};

// Before we have any clear idea on how this works, let's just play around
// with an attempt at building a minimum viable circuit.
//
// Update: Now we have a functioning gate, and a test to show that it works.
// TODO: This main should either be completed to something that makes sense,
// or it should be discarded.
fn main() {
    // A common combination of fields and hashers to use for plonk in plonky2.
    type PlonkConfig = plonky2::plonk::config::PoseidonGoldilocksConfig;

    // This is the only field that natively implements the plonky2 trait `RichField`
    // The trait must be implemented in order to use a given field with plonky2.
    use plonky2::field::goldilocks_field::GoldilocksField;

    // TODO: WTF is this, and how should we set it?
    //
    // Some observations:
    // `plonky2::field::goldilocks_extensions` implements
    // `Extendable<2>`, `Extendable<4>`, `Extendable<5>` for
    // `GoldilocksField`. This suggests that, if we want to
    // use this field, it is highly advisable that we set
    // `FIELD_EXTENSION_DEGREE` to either 2, 4, or 5.
    //
    // The type `plonky2::plonk::config::PoseidonGoldilocksConfig`
    // suggests we use 2. Why? No clue yet.
    const FIELD_EXTENSION_DEGREE: usize = 2;

    // It's a good thing that standard configurations exist, but
    // TODO: maybe let's try understanding how they work sometime soon
    let circuit_config =
        plonky2::plonk::circuit_data::CircuitConfig::standard_recursion_zk_config();

    let mut builder = plonky2::plonk::circuit_builder::CircuitBuilder::<
        GoldilocksField,
        FIELD_EXTENSION_DEGREE,
    >::new(circuit_config);

    let a = builder.add_virtual_target();
}
