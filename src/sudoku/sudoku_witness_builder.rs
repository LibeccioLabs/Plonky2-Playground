use std::collections::HashMap;

use plonky2::{
    field::types::Field,
    iop::witness::{PartialWitness, Witness, WitnessWrite},
};

use crate::permutation::{DefaultSwapSchedule, SwapSchedule};

use super::SudokuProblemTarget;

pub enum SudokuWitnessBuilder<const SIZE: usize, const SIZE_SQRT: usize> {}

impl<const SIZE: usize, const SIZE_SQRT: usize> SudokuWitnessBuilder<SIZE, SIZE_SQRT> {
    /// This function sets the problem grids for a sudoku and then fills the
    /// advice values for the permutation gates associated to the sudoku cirucit.
    pub fn set_sudoku_witness<F: Field>(
        witness: &mut PartialWitness<F>,
        sudoku_target: &SudokuProblemTarget<SIZE, SIZE_SQRT>,
        problem: [[usize; SIZE]; SIZE],
        solution: [[usize; SIZE]; SIZE],
    ) {
        let symbols: [_; SIZE] = core::array::from_fn(|n| F::from_canonical_usize(n + 1));
        let problem = problem.map(|row| {
            row.map(|n| match n {
                0 => F::ZERO,
                n => symbols[n - 1],
            })
        });
        let solution = solution.map(|row| row.map(|n| symbols[n - 1]));

        for (row_targets, row_values) in sudoku_target.problem.into_iter().zip(problem) {
            witness.set_target_arr(&row_targets, &row_values);
        }

        for (row_targets, row_values) in sudoku_target.solution.into_iter().zip(solution) {
            witness.set_target_arr(&row_targets, &row_values);
        }

        Self::compute_swap_selectors(witness, sudoku_target).expect("compute swap selectors fails if the problem and solution grids have not been set. We just set them.");
    }

    /// This function sets the advice values for the permutation gates
    /// associated to a sudoku circuit.
    /// The function assumes that the problem and solution grids have been
    /// set beforehand, and fails otherwise.
    pub fn compute_swap_selectors<F: Field>(
        witness: &mut PartialWitness<F>,
        sudoku_target: &SudokuProblemTarget<SIZE, SIZE_SQRT>,
    ) -> Result<(), ()> {
        let symbols_to_usize: HashMap<F, usize, std::collections::hash_map::RandomState> =
            HashMap::from_iter((0..SIZE).map(|n| (F::from_canonical_usize(n + 1), n)));

        let mut solution = [[0; SIZE]; SIZE];

        for row_idx in 0..SIZE {
            for col_idx in 0..SIZE {
                solution[row_idx][col_idx] = *symbols_to_usize
                    .get(
                        &witness
                            .try_get_target(sudoku_target.solution[row_idx][col_idx])
                            // None if the target has not been set in the witness yet.
                            .ok_or(())?,
                    )
                    // None if the target value in the witness is not a valid symbol.
                    .ok_or(())?;
            }
        }

        fn compute_swap_selectors<F: Field>(permutation: &mut [usize]) -> impl Iterator<Item = F> {
            DefaultSwapSchedule::permutation_to_swap_schedule(permutation)
                .into_iter()
                .map(|(selector, _idx1, _idx2)| F::from_bool(selector))
        }

        // Computing swap selectors for permutations on rows.
        // For every row we have a number of targets to set to `F::ZERO` or `F::ONE`.
        // `F` provides the `from_bool` method to convert from bool values.
        //
        // Given a row, we want to show it is a permutation of `0..SIZE`.
        // and in order to do it we have to provide the witness values for the
        // permutation gate. We can get the witness values by feeding the values of
        // the cells in the row to the `compute_swap_schedule` function.
        //
        // The appropriate target positions used in the gate were saved un the
        // `sudoku_target` struct at circuit creation time.
        for (selector_targets, mut row) in sudoku_target
            .row_swap_selectors
            .iter()
            .zip(SudokuProblemTarget::get_rows(&solution))
        {
            for (target, selector_value) in selector_targets
                .into_iter()
                .zip(compute_swap_selectors(&mut row))
            {
                witness.set_target(*target, selector_value);
            }
        }

        // Computing swap selectors for permutations on columns.
        // Analogous to what we did on rows.
        for (selector_targets, mut column) in sudoku_target
            .column_swap_selectors
            .iter()
            .zip(SudokuProblemTarget::get_columns(&solution))
        {
            for (target, selector_value) in selector_targets
                .into_iter()
                .zip(compute_swap_selectors(&mut column))
            {
                witness.set_target(*target, selector_value);
            }
        }

        // Computing swap selectors for permutations on regions.
        // Analogous to what we did on rows and columns.
        for (selector_targets, mut region) in sudoku_target.region_swap_selectors.iter().zip(
            SudokuProblemTarget::<SIZE, SIZE_SQRT>::get_regions(&solution),
        ) {
            for (target, selector_value) in selector_targets
                .into_iter()
                .zip(compute_swap_selectors(&mut region))
            {
                witness.set_target(*target, selector_value);
            }
        }

        Ok(())
    }
}
