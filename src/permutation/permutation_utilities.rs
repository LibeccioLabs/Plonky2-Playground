use std::{
    cell::{BorrowMutError, RefCell, RefMut},
    marker::PhantomData,
};

/// A struct that, given a buffer to write on, iterates over all the
/// permutations of a given length.
///
/// Notice that the iterator yields RefCell guards to the underlying
/// buffer, therefore only one iteration item can be used at any given time.
///
/// To use more items at once, their result must be copied in other
/// memory locations, and the RefCell guards must go out of scope.
pub struct PermutationsIter<'a: 'i, 'i> {
    iterator_output: &'a mut [usize],
    pd: PhantomData<&'i ()>,
}

impl<'a: 'i, 'i> From<&'a mut [usize]> for PermutationsIter<'a, 'i> {
    fn from(value: &'a mut [usize]) -> Self {
        Self {
            iterator_output: value,
            pd: PhantomData,
        }
    }
}

impl<'a: 'i, 'i> IntoIterator for PermutationsIter<'a, 'i> {
    type IntoIter = KnuthL<'a, 'i>;
    type Item = <KnuthL<'a, 'i> as Iterator>::Item;
    fn into_iter(self) -> Self::IntoIter {
        KnuthL::new(self.iterator_output)
    }
}

/// A struct that iterates over all the permutations of a given length.
pub struct KnuthL<'a: 'i, 'i> {
    permutation: Option<Vec<usize>>,
    iterator_output: std::cell::RefCell<&'a mut [usize]>,
    pd: PhantomData<&'i ()>,
}

impl<'a: 'i, 'i> KnuthL<'a, 'i> {
    fn new(iterator_output: &'a mut [usize]) -> Self {
        let n_objects = iterator_output.len();
        let iterator_output = RefCell::new(iterator_output);
        if n_objects == 0 {
            return Self {
                permutation: None,
                iterator_output,
                pd: PhantomData,
            };
        }

        let permutation = Some({
            let mut v = Vec::with_capacity(n_objects);
            v.extend(0..n_objects);
            v
        });

        Self {
            permutation,
            iterator_output,
            pd: PhantomData,
        }
    }
}

impl<'a, 'i> Iterator for KnuthL<'a, 'i> {
    type Item = Result<RefMut<'i, &'a mut [usize]>, BorrowMutError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.permutation == None {
            return None;
        }

        // Try borrowing the output buffer. If the borrow operation fails,
        // Some(Err(...)) is returned, and the iterator's state is left unchanged.
        let output = self.iterator_output.try_borrow_mut();

        let mut output = output.map(|borrow| unsafe {
            // About the unsafe block: the data sctucture we want to use as iteration item
            // cannot have a generic lifetime, since it contains a `'a` lived pointer.
            // We can safely make the compiler believe that the returned variable lives `'i`
            // because the underlying data lives `'a`, which outlives `'i`
            // and the compiler will deduce `'i` form the scope the output items live in.
            //
            // Also, this function does not actually need type annotations,
            // but we want to avoid pesky bugs that could emerge with refactoring
            // if the annotations weren't there.
            core::mem::transmute::<RefMut<'_, &'a mut [usize]>, RefMut<'i, &'a mut [usize]>>(borrow)
        });

        if output.is_err() {
            return Some(output);
        }

        let array = self
            .permutation
            .as_mut()
            .expect("we checked at the beginning that self.permutation != None")
            .as_mut_slice();

        let n_objects = array.len();

        // Copy the current state, to output buffer.
        output
            .as_mut()
            .expect("If output was Err, we would have returned it by now.")
            .copy_from_slice(array);

        // Find last j such that self[j] <= self[j+1].
        // Nullify self.0 if it doesn't exist
        let j = (0..=n_objects - 2)
            .rev()
            .find(|&j| array[j] <= array[j + 1]);

        // The last permutation we yield is [N_OBJECTS - 1, N_OBJECTS - 2, ..., 1, 0]
        if j == None {
            self.permutation = None;
            return Some(output);
        }

        let j = j.unwrap();

        // Find last l such that self[j] <= self[l], then
        // exchange elements j and l, and then reverse self[j+1..]
        let l = (j + 1..=n_objects - 1).rev().find(|&l| array[j] <= array[l])
        .expect("since `j + 1` is in the range, and given the definition of `j`, we are sure that `find` will return `Some(...)`");
        array.swap(j, l);
        array[j + 1..].reverse();

        Some(output)
    }
}

/// Given a permutation, writes its inverse to `output_buffer`.
///
/// Warning: no forms of error checking are in place.
/// It is up to the caller to guarantee that the input
/// to this function is an actual permutation.
pub fn inverse_permutation(permutation: &[usize], output_buffer: &mut [usize]) {
    for (i, n) in permutation.into_iter().map(|n| *n).enumerate() {
        output_buffer[n] = i;
    }
}
