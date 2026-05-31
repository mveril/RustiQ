#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PairIndex(pub usize);

impl PairIndex {
    pub fn new(i: usize, j: usize) -> Self {
        let (a, b) = if i >= j { (i, j) } else { (j, i) };

        Self(a * (a + 1) / 2 + b)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EriIndex(pub usize);
impl EriIndex {
    pub fn new(mu: usize, nu: usize, lambda: usize, sigma: usize) -> EriIndex {
        let PairIndex(p) = PairIndex::new(mu, nu);
        let PairIndex(q) = PairIndex::new(lambda, sigma);

        let (a, b) = if p >= q { (p, q) } else { (q, p) };

        EriIndex(a * (a + 1) / 2 + b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_pair_index_matches_triangular_layout() {
        let mut expected = 0;
        for i in 0..5 {
            for j in 0..=i {
                assert_eq!(PairIndex::new(i, j), PairIndex(expected));
                assert_eq!(PairIndex::new(j, i), PairIndex(expected));
                expected += 1;
            }
        }
    }

    #[test]
    fn test_eri_index_matches_pair_of_pairs_triangular_layout() {
        let pair_count = 10;
        let mut expected = 0;
        for p in 0..pair_count {
            for q in 0..=p {
                let (mu, nu) = basis_function_pair(p);
                let (lambda, sigma) = basis_function_pair(q);

                assert_eq!(EriIndex::new(mu, nu, lambda, sigma), EriIndex(expected));
                assert_eq!(EriIndex::new(lambda, sigma, mu, nu), EriIndex(expected));
                expected += 1;
            }
        }
    }

    #[test]
    fn test_eri_index_covers_unique_quartets_without_gaps() {
        let basis_functions = 5;
        let pair_count = basis_functions * (basis_functions + 1) / 2;
        let unique_quartets = pair_count * (pair_count + 1) / 2;
        let mut seen = HashSet::new();

        for mu in 0..basis_functions {
            for nu in 0..basis_functions {
                for lambda in 0..basis_functions {
                    for sigma in 0..basis_functions {
                        seen.insert(EriIndex::new(mu, nu, lambda, sigma).0);
                    }
                }
            }
        }

        assert_eq!(seen.len(), unique_quartets);
        for index in 0..unique_quartets {
            assert!(seen.contains(&index), "missing compact ERI index {index}");
        }
    }

    fn basis_function_pair(pair_index: usize) -> (usize, usize) {
        let first = (((8 * pair_index + 1) as f64).sqrt() as usize - 1) / 2;
        let second = pair_index - first * (first + 1) / 2;
        (first, second)
    }
}
