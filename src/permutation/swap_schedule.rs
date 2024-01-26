pub trait SwapSchedule {
    /// Given a number `n_objects`, it outputs a sequence of
    /// swaps that would be attempted on a `n_objects` long vector.
    fn get_swap_sequence(n_objects: usize) -> Vec<(usize, usize)> {
        let mut identity_permutation = Vec::with_capacity(n_objects);
        identity_permutation.extend(0..n_objects);
        Self::permutation_to_swap_schedule(&mut identity_permutation)
            .into_iter()
            .map(|(_selector, idx1, idx2)| (idx1, idx2))
            .collect()
    }

    /// Given `permutation`, a permutation on `permutation.len()` objects,
    /// if outputs a selection of the swaps provided by `Self::get_swap_sequence`,
    /// so that the composition of the selected swaps amounts to the input permutation.
    ///
    /// Assumptions: for every permutation, there exists a valid selection.
    /// If this is not the case, the implementation should crash
    /// when there is no valid selections.
    fn permutation_to_swap_schedule(permutation: &mut [usize]) -> Vec<(bool, usize, usize)>;
}

pub type DefaultSwapSchedule = RecusriveSplitTwoSchedule;

/// The sequence of swaps attempted by the bubble sort algorithm.
/// It has quadratic complexity in the permutation length.
pub enum BubbleSortSwapSchedule {}

/// Given a permutation, it outputs a vector consisting
/// of `(bool, usize, usize)` triplets. Each triplet represents a swap that
/// may or may not happen between the items at the usize positions in the tuple.
///
/// This schedule has sub-quadratic complexity in the length of `permutation`.
/// Precisely, O( permutation.len() ** (ln(3)/ln(2)) ) < O(permutation.len() ** 1.6)
pub enum RecusriveSplitTwoSchedule {}

impl SwapSchedule for BubbleSortSwapSchedule {
    fn permutation_to_swap_schedule(permutation: &mut [usize]) -> Vec<(bool, usize, usize)> {
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
}

impl SwapSchedule for RecusriveSplitTwoSchedule {
    fn permutation_to_swap_schedule(permutation: &mut [usize]) -> Vec<(bool, usize, usize)> {
        recursive_permutation_to_2_split_schedule(permutation, 0, vec![])
    }
}

fn recursive_permutation_to_2_split_schedule(
    permutation: &mut [usize],
    offset: usize,
    mut out: Vec<(bool, usize, usize)>,
) -> Vec<(bool, usize, usize)> {
    let len = permutation.len();
    match len {
        0 | 1 => return out,
        2 => {
            out.push((
                if permutation[0] > permutation[1] {
                    permutation.swap(0, 1);
                    true
                } else {
                    false
                },
                offset,
                offset + 1,
            ));
            return out;
        }
        _ => (),
    }

    let split_idx = (len + 1) / 2;
    let part2_offset = offset + split_idx;
    let (part1, part2) = permutation.split_at_mut(split_idx);
    // we track the indices `i` such that `part2[i]` "belongs to part1".
    // then, we compute a permutation of part1 such that after its application,
    // for every `i` as above, `part1[i]` "belongs to part2".
    //
    // We initialize the permutation to identity.
    let mut prepare_part1 = Vec::from_iter(0..part1.len());

    // we need an auxiliary index. We will use it to traverse part1
    let mut idx1: usize = 0;

    for idx2 in 0..part2.len() {
        // if `part2[idx2]` and `part1[idx2]` both "belong to part1",
        // we have to rearrange `part1[idx2]` using `prepare_part1`
        // before applying the swaps between `part1` and `part2`.
        if part2[idx2] < part2_offset && part1[idx2] < part2_offset {
            // so we search for a suitable item in `part1` that
            // "belongs to part2" to swap with `part1[idx2]`
            loop {
                // if `part1[idx1]` and `part2[idx1]` "belong to part2",
                // then we can swap `part1[idx2]` and `part1[idx1]`
                //
                // in the edge case where `idx1 == part2.len()`,
                // we simply ask that `part1[idx1]` "belongs to part2".
                if part1[idx1] >= part2_offset
                    && (idx1 == part2.len() || part2[idx1] >= part2_offset)
                {
                    part1.swap(idx1, idx2);
                    // the next two lines are equivalent to
                    // `prepare_part1.swap(idx1, idx2)`
                    //
                    // but we can be more efficient than that because
                    // we know that `prepare_part1` is made of disjoint
                    // swaps, and the beginning state of `prepare_part1`
                    // was such that, for all `i`, `prepare_part1[i] == i`
                    prepare_part1[idx1] = idx2;
                    prepare_part1[idx2] = idx1;

                    idx1 = idx1 + 1;
                    break;
                }

                idx1 = idx1 + 1;
            }
        }
    }

    // now we want to apply the `prepare_part1` permutation to `part1`.
    // to this end, we compute the schedule for `prepare_part1`,
    // and we adapt it taking into account the fact that `part1` is at
    // an offset with respect to a bigger slice.
    let mut prepare_part1_schedule =
        recursive_permutation_to_2_split_schedule(&mut prepare_part1, 0, vec![]);
    for (_, i, j) in prepare_part1_schedule.iter_mut() {
        *i = *i + offset;
        *j = *j + offset;
    }
    out.extend_from_slice(&prepare_part1_schedule);

    // Then we apply swaps among items of `part1` and `part2`
    // with same relative index, to migrate all the items that
    // "belong to part1" to `part1`, and all the items that
    // "belong to part2" to `part2`.
    for idx in 0..part2.len() {
        out.push((
            if part2[idx] < part2_offset {
                let (i1, i2) = (part1[idx], part2[idx]);
                part1[idx] = i2;
                part2[idx] = i1;

                true
            } else {
                false
            },
            idx + offset,
            idx + part2_offset,
        ));
    }

    //then, we separately append the permutation schedule for `part1` and `part2`
    let out = recursive_permutation_to_2_split_schedule(part1, offset, out);
    let out = recursive_permutation_to_2_split_schedule(part2, part2_offset, out);

    out
}

#[test]
fn test_recursive_2_split_schedule() {
    let mut permutation = [1, 3, 8, 4, 2, 5, 0, 7, 6];
    println!(
        "{:?}",
        RecusriveSplitTwoSchedule::permutation_to_swap_schedule(&mut permutation)
    );
}
