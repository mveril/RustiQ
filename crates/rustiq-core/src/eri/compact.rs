use std::ops::{Index, IndexMut};

use rayon::prelude::*;
use thiserror::Error;

use super::index::EriIndex;

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum CompactEriBuildError {
    #[error("expected {expected} compact ERI values, received {actual}")]
    InvalidLength { expected: usize, actual: usize },
}

#[derive(Debug)]
pub struct CompactEri {
    storage: Box<[f64]>,
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
            storage: (0..Self::storage_len(size)).map(|_| 0.0).collect(),
        }
    }

    /// Builds a compact ERI tensor from values yielded in compact storage order.
    pub(crate) fn from_ordered_values_par_iter<I>(
        size: usize,
        par_iter: I,
    ) -> Result<Self, CompactEriBuildError>
    where
        I: IndexedParallelIterator<Item = f64>,
    {
        let storage_len = Self::storage_len(size);
        let actual = par_iter.len();
        if actual != storage_len {
            return Err(CompactEriBuildError::InvalidLength {
                expected: storage_len,
                actual,
            });
        }

        let storage = par_iter.collect();

        Ok(Self { storage })
    }
}

impl Index<EriIndex> for CompactEri {
    type Output = f64;

    fn index(&self, index: EriIndex) -> &Self::Output {
        &self.storage[index.0]
    }
}

impl IndexMut<EriIndex> for CompactEri {
    fn index_mut(&mut self, index: EriIndex) -> &mut Self::Output {
        &mut self.storage[index.0]
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
        &mut self.storage[EriIndex::new(mu, nu, lambda, sigma).0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eri::index::PairIndex;
    use ndarray::Array4;
    use proptest::prelude::*;

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
                let (pair_pq, pair_rs) = PairIndex(compact_index).indices();
                let (mu, nu) = PairIndex(pair_pq).indices();
                let (lambda, sigma) = PairIndex(pair_rs).indices();

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
    fn test_compact_eri_from_ordered_values_par_iter_uses_compact_order() {
        let basis_functions = 5;
        let storage_len = CompactEri::storage_len(basis_functions);

        let eri = CompactEri::from_ordered_values_par_iter(
            basis_functions,
            (0..storage_len)
                .into_par_iter()
                .map(|compact_index| compact_index as f64 + 0.25),
        )
        .unwrap();

        assert_eq!(eri[(0, 0, 0, 0)], 0.25);
        assert_eq!(eri[(1, 0, 0, 0)], 1.25);
        assert_eq!(eri[(1, 0, 1, 0)], 2.25);
        assert_eq!(
            eri[(4, 4, 4, 4)],
            CompactEri::storage_len(basis_functions) as f64 - 0.75
        );
    }

    proptest! {
        #[test]
        fn test_compact_eri_from_ordered_values_par_iter_rejects_invalid_lengths(
            basis_functions in 0usize..=8,
            values in prop::collection::vec(any::<f64>(), 0..300),
        ) {
            let expected = CompactEri::storage_len(basis_functions);
            let actual = values.len();
            prop_assume!(actual != expected);

            let error = CompactEri::from_ordered_values_par_iter(
                basis_functions,
                values.into_par_iter(),
            )
            .unwrap_err();

            prop_assert_eq!(
                error,
                CompactEriBuildError::InvalidLength { expected, actual }
            );
        }
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
}
