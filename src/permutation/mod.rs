use plonky2::iop::target::BoolTarget;

mod gate;
pub use gate::{ApplyPermutation, PermutationGate};

mod swap_schedule;
pub use swap_schedule::{
    BubbleSortSwapSchedule, DefaultSwapSchedule, RecusriveSplitTwoSchedule, SwapSchedule,
};

mod permutation_utilities;
pub use permutation_utilities::{inverse_permutation, KnuthL, PermutationsIter};

#[derive(Debug, Clone, Copy)]
pub struct SwapIndexOutOfRange {
    pub max_allowed: usize,
    pub found: (Option<BoolTarget>, usize, usize),
}
