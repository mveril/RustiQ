use std::ops::{Index, IndexMut};

use super::EriIndex;

pub struct CompactEri {
    storage: Box<[f64]>,
}

impl CompactEri {
    pub fn Zeroed(size: usize) -> Self {
        let storage_len = if size == 0 {
            0
        } else {
            EriIndex::new(size - 1, size - 1, size - 1, size - 1).0 + 1
        };
        Self {
            storage: vec![0f64; storage_len].into_boxed_slice(),
        }
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
    use ndarray::Array4;

    #[test]
    fn test_compact_eri_allocates_unique_quartets() {
        let basis_functions = 4;
        let pair_count = basis_functions * (basis_functions + 1) / 2;
        let unique_quartets = pair_count * (pair_count + 1) / 2;

        let eri = CompactEri::Zeroed(basis_functions);

        assert_eq!(eri.storage.len(), unique_quartets);
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
