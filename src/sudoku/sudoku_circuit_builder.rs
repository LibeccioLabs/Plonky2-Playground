use crate::permutation::ApplyPermutation;

use super::SudokuProblemTarget;

use plonky2::{
    field::extension::Extendable, hash::hash_types::RichField,
    plonk::circuit_builder::CircuitBuilder,
};

pub enum SudokuCircuitBuilder<const SIZE: usize, const SIZE_SQRT: usize> {}

impl<const SIZE: usize, const SIZE_SQRT: usize> SudokuCircuitBuilder<SIZE, SIZE_SQRT> {
    pub fn add_proof_of_sudoku_solution<const D: usize, F: RichField + Extendable<D>>(
        builder: &mut CircuitBuilder<F, D>,
    ) -> Result<SudokuProblemTarget<SIZE, SIZE_SQRT>, ()> {
        // Rust's constant computations don't allow to perform operations
        // on generic constants yet, so to get the square root of SIZE we
        // are stuck with this ugliness.
        assert_eq!(SIZE_SQRT * SIZE_SQRT, SIZE);

        let schedule_length = builder.permutation_swap_schedule_length(SIZE);

        let out = SudokuProblemTarget {
            problem: core::array::from_fn(|_| builder.add_virtual_target_arr()),
            solution: core::array::from_fn(|_| builder.add_virtual_target_arr()),
            row_swap_selectors: core::array::from_fn(|_| {
                builder.add_virtual_targets(schedule_length)
            }),
            column_swap_selectors: core::array::from_fn(|_| {
                builder.add_virtual_targets(schedule_length)
            }),
            region_swap_selectors: core::array::from_fn(|_| {
                builder.add_virtual_targets(schedule_length)
            }),
        };

        // Symbols are `1 ..= SIZE`. `0` is reserved to the values in the
        // problem grid, where it means "the cell is empty".
        let symbols: [_; SIZE] =
            core::array::from_fn(|idx| builder.constant(F::from_canonical_usize(idx + 1)));

        // Applying row constraints to the solution
        for (row, selectors) in SudokuProblemTarget::get_rows(&out.solution)
            .iter()
            .zip(out.row_swap_selectors.iter())
        {
            builder.add_permutation_gate(row, selectors, &symbols, true)?;
        }

        // Applying column constraints to the solution
        for (column, selectors) in SudokuProblemTarget::get_columns(&out.solution)
            .iter()
            .zip(out.column_swap_selectors.iter())
        {
            builder.add_permutation_gate(column, selectors, &symbols, true)?;
        }

        // Applying region constraints to the solution
        for (region, selectors) in
            SudokuProblemTarget::<SIZE, SIZE_SQRT>::get_regions(&out.solution)
                .iter()
                .zip(out.region_swap_selectors.iter())
        {
            builder.add_permutation_gate(region, selectors, &symbols, true)?;
        }

        // We enforce the constraint that, for all i < SIZE and all j < SIZE,
        //  if `problem[i][j] != 0`, then `problem[i][j] == solution[i][j]`.
        for (problem_row, solution_row) in out.problem.into_iter().zip(out.solution) {
            for (problem_cell, solution_cell) in problem_row.into_iter().zip(solution_row) {
                // we want to enforce
                // `problem_cell * (problem_cell - solution_cell) == 0`
                let delta = builder.sub(problem_cell, solution_cell);
                let constraint = builder.mul(problem_cell, delta);
                builder.assert_zero(constraint);
            }
        }

        Ok(out)
    }
}
