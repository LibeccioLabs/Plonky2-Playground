use crate::permutation::ApplyPermutation;

use super::SudokuProblemTarget;

use plonky2::{
    field::extension::Extendable, hash::hash_types::RichField, plonk::circuit_builder::CircuitBuilder
};

pub enum SudokuCircuitBuilder<const SIZE: usize, const SIZE_SQRT: usize> {}

impl<const SIZE: usize, const SIZE_SQRT: usize> SudokuCircuitBuilder<SIZE, SIZE_SQRT> {

    pub fn add_proof_of_sudoku_solution<const D: usize, F: RichField + Extendable<D>>(
        builder: &mut CircuitBuilder<F, D>,
    ) -> Result<SudokuProblemTarget<SIZE, SIZE_SQRT>, ()> {
        add_proof_of_sudoku_solution_helper::<false, SIZE, SIZE_SQRT, D, F>(builder)
    }

    /// Like `add_proof_of_sudoku_solution`, but the circuit is slightly less
    /// optimized, in a way that allows proof generation not to cause a panic
    /// when an invalid witness is input into the circuit.
    ///
    /// Needed when we want to check that the circuit logic actually forbids
    /// the generation of bogous proofs.
    #[cfg(test)]
    pub fn add_proof_of_sudoku_solution_fail_gracefully<const D: usize, F: RichField + Extendable<D>>(
        builder: &mut CircuitBuilder<F, D>,
    ) -> Result<SudokuProblemTarget<SIZE, SIZE_SQRT>, ()> {
        add_proof_of_sudoku_solution_helper::<true, SIZE, SIZE_SQRT, D, F>(builder)
    }
}

fn add_proof_of_sudoku_solution_helper<
    const TEST_MODE: bool,
    const SIZE: usize,
    const SIZE_SQRT: usize,
    const D: usize,
    F: RichField + Extendable<D>,
>(
    builder: &mut CircuitBuilder<F, D>,
) -> Result<SudokuProblemTarget<SIZE, SIZE_SQRT>, ()> {
    // Rust's constant computations don't allow to perform operations
    // on generic constants yet, so to get the square root of SIZE we
    // are stuck with this ugliness.
    assert_eq!(SIZE_SQRT * SIZE_SQRT, SIZE);

    // If TEST_MODE is true, we will use
    // crate::utilities::test_connect_gate::TestEq::connect
    // instead of the usual
    // CircuitBuilder::connect
    // to make two cells equal in a circuit.
    // This is done to avoid panics if the witness is invalid.
    //
    // The standard config has 80 routable wires, and the
    // TestEq gate needs 2 wires per operation.
    const N_TEST_EQ_OPS: usize = 80 / 2;


    let schedule_length = builder.permutation_swap_schedule_length(SIZE);

    let out = SudokuProblemTarget {
        problem: core::array::from_fn(|_| builder.add_virtual_target_arr()),
        solution: core::array::from_fn(|_| builder.add_virtual_target_arr()),
        symbols: 
            // Symbols are `1 ..= SIZE`. `0` is reserved to the values in the
            // problem grid, where it means "the cell is empty".
            core::array::from_fn(|idx| builder.constant(F::from_canonical_usize(idx + 1)))
        ,
        row_swap_selectors: core::array::from_fn(|_| builder.add_virtual_targets(schedule_length)),
        column_swap_selectors: core::array::from_fn(|_| {
            builder.add_virtual_targets(schedule_length)
        }),
        region_swap_selectors: core::array::from_fn(|_| {
            builder.add_virtual_targets(schedule_length)
        }),
    };

    for (group, selectors) in
        // Applying row constraints to the solution
        SudokuProblemTarget::get_rows(&out.solution)
            .iter()
            .zip(out.row_swap_selectors.iter())
        .chain(
            // Applying column constraints to the solution
            SudokuProblemTarget::get_columns(&out.solution)
            .iter()
            .zip(out.column_swap_selectors.iter())
        ).chain(
            // Applying region constraints to the solution
            SudokuProblemTarget::<SIZE, SIZE_SQRT>::get_regions(&out.solution)
            .iter()
            .zip(out.region_swap_selectors.iter())
        )
    {
        let out_targets = builder.add_virtual_target_arr::<SIZE>();
        builder.add_permutation_gate(group, selectors, &out_targets, true)?;
        
        // if we are in test mode, we will add test-eq constraints.
        // those constraints are logically equivalent to `CircuitBuilder::connect`
        // but they don't cause a panic with an invalid witness.
        
            for (lhs, rhs) in out_targets.into_iter().zip(out.symbols) {
                if TEST_MODE {
                    crate::utilities::test_connect_gate::TestEq::<N_TEST_EQ_OPS>::connect(builder, lhs, rhs);
                
            } else {
                builder.connect(lhs, rhs);
            }
        }
    }

    let zero_target = builder.zero();
    // We enforce the constraint that, for all i < SIZE and all j < SIZE,
    //  if `problem[i][j] != 0`, then `problem[i][j] == solution[i][j]`.
    for (problem_row, solution_row) in out.problem.into_iter().zip(out.solution) {
        for (problem_cell, solution_cell) in problem_row.into_iter().zip(solution_row) {
            // we want to enforce
            // `problem_cell * (problem_cell - solution_cell) == 0`
            let delta = builder.sub(problem_cell, solution_cell);
            let constraint = builder.mul(problem_cell, delta);

            // If we are in test mode, we use TestEq gate to avoid panics.
            if TEST_MODE {
                crate::utilities::test_connect_gate::TestEq::<N_TEST_EQ_OPS>::connect(builder, constraint, zero_target);
            } else {
                builder.assert_zero(constraint);
            }
        }
    }

    Ok(out)
}