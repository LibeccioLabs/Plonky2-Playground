/// Given a number `n_objects`, it outputs the sequence of
/// swaps that would be attempted by the bubblesort algorithm
/// on a `n_objects` long vector.
pub fn bubblesort_schedule(n_objects: usize) -> Vec<(usize, usize)> {
    let mut schedule = vec![];
    for i in (0..n_objects).rev() {
        for j in 0..i {
            schedule.push((j, j + 1));
        }
    }
    schedule
}

/// Given a permutation, it outputs a vector consisting
/// of `(bool, usize, usize)` triplets. Each triplet represents a swap that
/// may or may not happen between the items at the usize positions in the tuple.
pub fn from_permutation_to_bubble_sort_swap_schedule(
    permutation: &mut [usize],
) -> Vec<(bool, usize, usize)> {
    let n_objects = permutation.len();
    if n_objects < 2 {
        return vec![];
    }
    let mut bubble_sort_schedule = Vec::with_capacity(n_objects * (n_objects - 1) / 2);
    for i in (0..n_objects).rev() {
        for j in 0..i {
            bubble_sort_schedule.push((
                {
                    if permutation[j] > permutation[j + 1] {
                        permutation.swap(j, j + 1);
                        true
                    } else {
                        false
                    }
                },
                j,
                j + 1,
            ));
        }
    }

    bubble_sort_schedule
}
