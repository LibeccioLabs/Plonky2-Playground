use plonky2::iop::target::Target;

mod sudoku_circuit_builder;
pub use sudoku_circuit_builder::SudokuCircuitBuilder;

mod sudoku_witness_builder;
pub use sudoku_witness_builder::SudokuWitnessBuilder;

#[cfg(test)]
mod tests;

pub struct SudokuProblemTarget<const SIZE: usize, const SIZE_SQRT: usize> {
    problem: [[Target; SIZE]; SIZE],
    solution: [[Target; SIZE]; SIZE],
    symbols: [Target; SIZE],
    row_swap_selectors: [Vec<Target>; SIZE],
    column_swap_selectors: [Vec<Target>; SIZE],
    region_swap_selectors: [Vec<Target>; SIZE],
}

impl<const SIZE: usize> SudokuProblemTarget<SIZE, 0> {
    pub fn get_rows<Item: Copy>(grid: &[[Item; SIZE]; SIZE]) -> [[Item; SIZE]; SIZE] {
        *grid
    }

    pub fn get_columns<Item: Copy>(grid: &[[Item; SIZE]; SIZE]) -> [[Item; SIZE]; SIZE] {
        core::array::from_fn::<_, SIZE, _>(|col_idx| grid.map(|row| row[col_idx]))
    }
}

impl<const SIZE: usize, const SIZE_SQRT: usize> SudokuProblemTarget<SIZE, SIZE_SQRT> {
    pub fn get_regions<Item: Copy>(grid: &[[Item; SIZE]; SIZE]) -> [[Item; SIZE]; SIZE] {
        core::array::from_fn::<_, SIZE, _>(|region_idx| {
            let (row_region_offset, col_region_offset) = (
                SIZE_SQRT * (region_idx / SIZE_SQRT),
                SIZE_SQRT * (region_idx % SIZE_SQRT),
            );
            core::array::from_fn::<_, SIZE, _>(|cell_idx| {
                grid[row_region_offset + cell_idx / SIZE_SQRT]
                    [col_region_offset + cell_idx % SIZE_SQRT]
            })
        })
    }
}
