use std::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    ops::{Index, IndexMut},
    sync::atomic::{AtomicUsize, Ordering},
};

use rayon::prelude::*;
use thiserror::Error;

use super::index::EriIndex;

#[allow(dead_code)]
#[derive(Debug)]
struct AtomicBitmap {
    words: Box<[AtomicUsize]>,
}

#[allow(dead_code)]
impl AtomicBitmap {
    fn new(len: usize) -> Self {
        Self {
            words: (0..len.div_ceil(usize::BITS as usize))
                .into_par_iter()
                .map(|_| AtomicUsize::new(0))
                .collect(),
        }
    }

    fn claim(&self, index: usize) -> bool {
        let (word, mask) = Self::word_and_mask(index);
        self.words[word].fetch_or(mask, Ordering::Relaxed) & mask == 0
    }

    fn contains(&self, index: usize) -> bool {
        let (word, mask) = Self::word_and_mask(index);
        self.words[word].load(Ordering::Relaxed) & mask != 0
    }

    fn word_and_mask(index: usize) -> (usize, usize) {
        let bits_per_word = usize::BITS as usize;
        (index / bits_per_word, 1usize << (index % bits_per_word))
    }
}

#[derive(Debug)]
struct StorageSlot(UnsafeCell<MaybeUninit<f64>>);

impl StorageSlot {
    #[allow(dead_code)]
    fn uninitialized() -> Self {
        Self(UnsafeCell::new(MaybeUninit::uninit()))
    }

    fn initialized(value: f64) -> Self {
        Self(UnsafeCell::new(MaybeUninit::new(value)))
    }

    fn zeroed() -> Self {
        Self::initialized(0.0)
    }

    #[allow(dead_code)]
    unsafe fn write(&self, value: f64) {
        // The bitmap guarantees that only one thread writes this slot.
        unsafe { (*self.0.get()).write(value) };
    }

    unsafe fn get(&self) -> &f64 {
        // CompactEri is created only after every slot has been initialized.
        unsafe { (&*self.0.get()).assume_init_ref() }
    }

    unsafe fn get_mut(&mut self) -> &mut f64 {
        // Exclusive access to the slot guarantees exclusive access to the value.
        unsafe { (&mut *self.0.get()).assume_init_mut() }
    }
}

// Concurrent access is limited to write-once initialization guarded by the bitmap.
unsafe impl Sync for StorageSlot {}

#[allow(dead_code)]
#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum CompactEriBuildError {
    #[error("quartet ({mu}, {nu}, {lambda}, {sigma}) contains an index outside size {size}")]
    IndexOutOfBounds {
        mu: usize,
        nu: usize,
        lambda: usize,
        sigma: usize,
        size: usize,
    },
    #[error("quartet ({mu}, {nu}, {lambda}, {sigma}) occurs more than once")]
    DuplicateQuartet {
        mu: usize,
        nu: usize,
        lambda: usize,
        sigma: usize,
    },
    #[error("compact quartet at storage index {compact_index} is missing")]
    MissingQuartet { compact_index: usize },
}

#[derive(Debug)]
pub struct CompactEri {
    storage: Box<[StorageSlot]>,
}

impl CompactEri {
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.storage.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }

    pub(crate) fn storage_len(size: usize) -> usize {
        if size == 0 {
            0
        } else {
            EriIndex::new(size - 1, size - 1, size - 1, size - 1).0 + 1
        }
    }

    #[allow(dead_code)]
    pub fn Zeroed(size: usize) -> Self {
        Self {
            storage: (0..Self::storage_len(size))
                .map(|_| StorageSlot::zeroed())
                .collect(),
        }
    }

    /// Builds a compact ERI tensor from quartets yielded in compact storage order.
    #[allow(dead_code)]
    pub(crate) fn from_indexed_par_iter<I>(size: usize, par_iter: I) -> Self
    where
        I: IndexedParallelIterator<Item = (usize, usize, usize, usize, f64)>,
    {
        let storage_len = Self::storage_len(size);
        let mut storage = (0..storage_len)
            .map(|_| StorageSlot::zeroed())
            .collect::<Box<[_]>>();
        let compact_par_iter = par_iter
            .map(|(mu, nu, lambda, sigma, value)| (EriIndex::new(mu, nu, lambda, sigma).0, value));

        storage
            .par_iter_mut()
            .enumerate()
            .zip(compact_par_iter)
            .for_each(|((storage_index, slot), (compact_index, value))| {
                debug_assert_eq!(storage_index, compact_index);
                // The zipped indexed iterator gives exclusive access to each slot.
                unsafe { *slot.get_mut() = value };
            });

        Self { storage }
    }

    /// Builds a compact ERI tensor from values yielded in compact storage order.
    pub(crate) fn from_ordered_values_par_iter<I>(size: usize, par_iter: I) -> Self
    where
        I: IndexedParallelIterator<Item = f64>,
    {
        let storage_len = Self::storage_len(size);
        let storage = (0..storage_len)
            .into_par_iter()
            .map(|_| StorageSlot::uninitialized())
            .collect::<Box<[_]>>();

        par_iter.enumerate().for_each(|(index, value)| {
            // The indexed iterator yields each compact slot exactly once.
            unsafe {
                storage[index].write(value);
            }
        });

        Self { storage }
    }

    /// Builds a compact ERI tensor from quartets yielded in any order.
    ///
    /// The iterator must yield each unique compact quartet exactly once.
    #[allow(dead_code)]
    pub(crate) fn from_par_iter<I>(par_iter: I, size: usize) -> Result<Self, CompactEriBuildError>
    where
        I: IntoParallelIterator<Item = (usize, usize, usize, usize, f64)>,
    {
        let storage_len = Self::storage_len(size);
        let storage = (0..storage_len)
            .into_par_iter()
            .map(|_| StorageSlot::uninitialized())
            .collect::<Box<[_]>>();
        let bitmap = AtomicBitmap::new(storage_len);

        par_iter
            .into_par_iter()
            .try_for_each(|(mu, nu, lambda, sigma, value)| {
                if [mu, nu, lambda, sigma]
                    .into_iter()
                    .any(|index| index >= size)
                {
                    return Err(CompactEriBuildError::IndexOutOfBounds {
                        mu,
                        nu,
                        lambda,
                        sigma,
                        size,
                    });
                }

                let index = EriIndex::new(mu, nu, lambda, sigma).0;
                if !bitmap.claim(index) {
                    return Err(CompactEriBuildError::DuplicateQuartet {
                        mu,
                        nu,
                        lambda,
                        sigma,
                    });
                }

                // This thread owns the slot after setting its bitmap bit.
                unsafe { storage[index].write(value) };
                Ok(())
            })?;

        if let Some(compact_index) = (0..storage_len)
            .into_par_iter()
            .find_any(|&index| !bitmap.contains(index))
        {
            return Err(CompactEriBuildError::MissingQuartet { compact_index });
        }

        Ok(Self { storage })
    }
}

impl Index<EriIndex> for CompactEri {
    type Output = f64;

    fn index(&self, index: EriIndex) -> &Self::Output {
        // All slots are initialized before CompactEri is returned.
        unsafe { self.storage[index.0].get() }
    }
}

impl IndexMut<EriIndex> for CompactEri {
    fn index_mut(&mut self, index: EriIndex) -> &mut Self::Output {
        // All slots are initialized before CompactEri is returned.
        unsafe { self.storage[index.0].get_mut() }
    }
}

impl Index<(usize, usize, usize, usize)> for CompactEri {
    type Output = f64;

    fn index(&self, index: (usize, usize, usize, usize)) -> &Self::Output {
        let (mu, nu, lambda, sigma) = index;
        &self[EriIndex::new(mu, nu, lambda, sigma)]
    }
}

impl IndexMut<(usize, usize, usize, usize)> for CompactEri {
    fn index_mut(&mut self, index: (usize, usize, usize, usize)) -> &mut Self::Output {
        let (mu, nu, lambda, sigma) = index;
        // All slots are initialized before CompactEri is returned.
        unsafe { self.storage[EriIndex::new(mu, nu, lambda, sigma).0].get_mut() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array4;

    #[test]
    fn test_atomic_bitmap_claims_each_bit_once_across_word_boundaries() {
        let bitmap = AtomicBitmap::new(130);

        for index in [0, 63, 64, 127, 128, 129] {
            assert!(!bitmap.contains(index));
            assert!(bitmap.claim(index));
            assert!(bitmap.contains(index));
            assert!(!bitmap.claim(index));
        }
    }

    #[test]
    fn test_compact_eri_allocates_unique_quartets() {
        let basis_functions = 4;
        let pair_count = basis_functions * (basis_functions + 1) / 2;
        let unique_quartets = pair_count * (pair_count + 1) / 2;

        let eri = CompactEri::Zeroed(basis_functions);

        assert_eq!(eri.storage.len(), unique_quartets);
    }

    #[test]
    fn test_storage_len_matches_pair_of_pairs_triangular_count() {
        for basis_functions in 0..=256 {
            let pair_count = basis_functions * (basis_functions + 1) / 2;
            let unique_quartets = pair_count * (pair_count + 1) / 2;

            assert_eq!(CompactEri::storage_len(basis_functions), unique_quartets);
        }
    }

    #[test]
    fn test_compact_index_decoding_round_trips() {
        for basis_functions in 0..=64 {
            for compact_index in 0..CompactEri::storage_len(basis_functions) {
                let (pair_pq, pair_rs) = unique_pair_indices(compact_index);
                let (mu, nu) = basis_function_pair(pair_pq);
                let (lambda, sigma) = basis_function_pair(pair_rs);

                assert!(mu < basis_functions);
                assert!(nu <= mu);
                assert!(lambda < basis_functions);
                assert!(sigma <= lambda);
                assert_eq!(
                    EriIndex::new(mu, nu, lambda, sigma).0,
                    compact_index,
                    "round-trip mismatch for size {basis_functions} at {compact_index}"
                );
            }
        }
    }

    #[test]
    fn test_compact_eri_indexes_eightfold_symmetry() {
        let mut eri = CompactEri::Zeroed(4);

        eri[(0, 1, 2, 3)] = 42.0;

        for index in [
            (0, 1, 2, 3),
            (1, 0, 2, 3),
            (0, 1, 3, 2),
            (1, 0, 3, 2),
            (2, 3, 0, 1),
            (3, 2, 0, 1),
            (2, 3, 1, 0),
            (3, 2, 1, 0),
        ] {
            assert_eq!(eri[index], 42.0);
        }
    }

    #[test]
    fn test_compact_eri_matches_dense_array4_with_eri_symmetry() {
        let basis_functions = 5;
        let mut dense = Array4::zeros((
            basis_functions,
            basis_functions,
            basis_functions,
            basis_functions,
        ));
        let mut compact = CompactEri::Zeroed(basis_functions);

        for mu in 0..basis_functions {
            for nu in 0..=mu {
                for lambda in 0..basis_functions {
                    for sigma in 0..=lambda {
                        let pair_left = EriIndex::new(mu, nu, 0, 0).0;
                        let pair_right = EriIndex::new(lambda, sigma, 0, 0).0;
                        if pair_left < pair_right {
                            continue;
                        }

                        let value = unique_value(mu, nu, lambda, sigma);
                        compact[(mu, nu, lambda, sigma)] = value;
                        for index in eri_permutations(mu, nu, lambda, sigma) {
                            dense[index] = value;
                        }
                    }
                }
            }
        }

        for mu in 0..basis_functions {
            for nu in 0..basis_functions {
                for lambda in 0..basis_functions {
                    for sigma in 0..basis_functions {
                        assert_eq!(
                            compact[(mu, nu, lambda, sigma)],
                            dense[(mu, nu, lambda, sigma)],
                            "mismatch for ({mu}, {nu}, {lambda}, {sigma})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_compact_eri_from_par_iter_uses_four_indexes() {
        let basis_functions = 5;
        let storage_len = CompactEri::storage_len(basis_functions);

        let eri = CompactEri::from_par_iter(
            (0..storage_len)
                .into_par_iter()
                .rev()
                .filter(|_| true)
                .map(|compact_index| {
                    let (pair_pq, pair_rs) = unique_pair_indices(compact_index);
                    let (mu, nu) = basis_function_pair(pair_pq);
                    let (lambda, sigma) = basis_function_pair(pair_rs);
                    (mu, nu, lambda, sigma, compact_index as f64 + 0.25)
                }),
            basis_functions,
        )
        .unwrap();

        assert_eq!(eri[(0, 0, 0, 0)], 0.25);
        assert_eq!(eri[(1, 0, 0, 0)], 1.25);
        assert_eq!(eri[(1, 0, 1, 0)], 2.25);
        assert_eq!(eri[(1, 1, 1, 1)], 5.25);
        assert_eq!(
            eri[(4, 4, 4, 4)],
            CompactEri::storage_len(basis_functions) as f64 - 0.75
        );
    }

    #[test]
    fn test_compact_eri_from_indexed_par_iter_uses_four_indexes() {
        let basis_functions = 2;
        let storage_len = CompactEri::storage_len(basis_functions);

        let eri = CompactEri::from_indexed_par_iter(
            basis_functions,
            (0..storage_len).into_par_iter().map(|compact_index| {
                let (pair_pq, pair_rs) = unique_pair_indices(compact_index);
                let (mu, nu) = basis_function_pair(pair_pq);
                let (lambda, sigma) = basis_function_pair(pair_rs);
                (mu, nu, lambda, sigma, compact_index as f64 + 0.25)
            }),
        );

        assert_eq!(eri[(0, 0, 0, 0)], 0.25);
        assert_eq!(eri[(1, 0, 0, 0)], 1.25);
        assert_eq!(eri[(1, 0, 1, 0)], 2.25);
        assert_eq!(eri[(1, 1, 1, 1)], 5.25);
    }

    #[test]
    fn test_compact_eri_from_ordered_values_par_iter_uses_compact_order() {
        let basis_functions = 5;
        let storage_len = CompactEri::storage_len(basis_functions);

        let eri = CompactEri::from_ordered_values_par_iter(
            basis_functions,
            (0..storage_len)
                .into_par_iter()
                .map(|compact_index| compact_index as f64 + 0.25),
        );

        assert_eq!(eri[(0, 0, 0, 0)], 0.25);
        assert_eq!(eri[(1, 0, 0, 0)], 1.25);
        assert_eq!(eri[(1, 0, 1, 0)], 2.25);
        assert_eq!(
            eri[(4, 4, 4, 4)],
            CompactEri::storage_len(basis_functions) as f64 - 0.75
        );
    }

    #[test]
    fn test_compact_eri_from_par_iter_initializes_every_slot() {
        for basis_functions in 0..=8 {
            let storage_len = CompactEri::storage_len(basis_functions);

            for _ in 0..20 {
                let eri =
                    CompactEri::from_par_iter(
                        (0..storage_len).into_par_iter().rev().filter(|_| true).map(
                            |compact_index| {
                                let (pair_pq, pair_rs) = unique_pair_indices(compact_index);
                                let (mu, nu) = basis_function_pair(pair_pq);
                                let (lambda, sigma) = basis_function_pair(pair_rs);
                                (mu, nu, lambda, sigma, compact_index as f64 + 0.25)
                            },
                        ),
                        basis_functions,
                    )
                    .unwrap();

                for compact_index in 0..storage_len {
                    let (pair_pq, pair_rs) = unique_pair_indices(compact_index);
                    let (mu, nu) = basis_function_pair(pair_pq);
                    let (lambda, sigma) = basis_function_pair(pair_rs);
                    assert_eq!(eri[(mu, nu, lambda, sigma)], compact_index as f64 + 0.25);
                }
            }
        }
    }

    #[test]
    fn test_compact_eri_from_par_iter_rejects_duplicate_quartets() {
        let error = CompactEri::from_par_iter(
            vec![(0, 0, 0, 0, 1.0), (0, 0, 0, 0, 2.0)].into_par_iter(),
            1,
        )
        .unwrap_err();

        assert_eq!(
            error,
            CompactEriBuildError::DuplicateQuartet {
                mu: 0,
                nu: 0,
                lambda: 0,
                sigma: 0,
            }
        );
    }

    #[test]
    fn test_compact_eri_from_par_iter_rejects_missing_quartets() {
        let error = CompactEri::from_par_iter(Vec::new().into_par_iter(), 1).unwrap_err();

        assert_eq!(
            error,
            CompactEriBuildError::MissingQuartet { compact_index: 0 }
        );
    }

    #[test]
    fn test_compact_eri_from_par_iter_rejects_out_of_bounds_indexes() {
        let error =
            CompactEri::from_par_iter(vec![(1, 0, 0, 0, 1.0)].into_par_iter(), 1).unwrap_err();

        assert_eq!(
            error,
            CompactEriBuildError::IndexOutOfBounds {
                mu: 1,
                nu: 0,
                lambda: 0,
                sigma: 0,
                size: 1,
            }
        );
    }

    #[test]
    fn test_compact_eri_zero_size_has_no_storage() {
        let eri = CompactEri::Zeroed(0);

        assert!(eri.storage.is_empty());
    }

    fn unique_value(mu: usize, nu: usize, lambda: usize, sigma: usize) -> f64 {
        let compact_index = EriIndex::new(mu, nu, lambda, sigma).0;
        compact_index as f64 + 0.25
    }

    fn eri_permutations(
        mu: usize,
        nu: usize,
        lambda: usize,
        sigma: usize,
    ) -> [(usize, usize, usize, usize); 8] {
        [
            (mu, nu, lambda, sigma),
            (nu, mu, lambda, sigma),
            (mu, nu, sigma, lambda),
            (nu, mu, sigma, lambda),
            (lambda, sigma, mu, nu),
            (sigma, lambda, mu, nu),
            (lambda, sigma, nu, mu),
            (sigma, lambda, nu, mu),
        ]
    }

    fn unique_pair_indices(index: usize) -> (usize, usize) {
        let pair_pq = (((8 * index + 1) as f64).sqrt() as usize - 1) / 2;
        let pair_rs = index - pair_pq * (pair_pq + 1) / 2;
        (pair_pq, pair_rs)
    }

    fn basis_function_pair(pair_index: usize) -> (usize, usize) {
        let first = (((8 * pair_index + 1) as f64).sqrt() as usize - 1) / 2;
        let second = pair_index - first * (first + 1) / 2;
        (first, second)
    }
}
