use bounded_vec_deque::BoundedVecDeque;
use nalgebra::{DMatrix, DVector};
use rayon::prelude::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum DiisError {
    #[error("DIIS history size must be at least 2, got {0}")]
    HistoryTooSmall(usize),
}
#[derive(Debug, Clone)]
struct DiisEntry {
    fock_matrix: DMatrix<f64>,
    error_matrix: DMatrix<f64>,
}

#[derive(Debug, Clone)]
pub(crate) struct DiisAccelerator {
    history: BoundedVecDeque<DiisEntry>,
}

impl DiisAccelerator {
    pub(crate) fn try_new(max_history: usize) -> Result<Self, DiisError> {
        if max_history < 2 {
            return Err(DiisError::HistoryTooSmall(max_history));
        }

        Ok(Self {
            history: BoundedVecDeque::new(max_history),
        })
    }

    pub(crate) fn extrapolate(
        &mut self,
        fock_matrix: &DMatrix<f64>,
        density_matrix: &DMatrix<f64>,
        overlap_matrix: &DMatrix<f64>,
    ) -> Option<DMatrix<f64>> {
        let error_matrix = Self::error_matrix(fock_matrix, density_matrix, overlap_matrix);
        self.push_history(fock_matrix.clone(), error_matrix);
        self.extrapolated_fock_matrix()
    }

    pub(crate) fn error_matrix(
        fock_matrix: &DMatrix<f64>,
        density_matrix: &DMatrix<f64>,
        overlap_matrix: &DMatrix<f64>,
    ) -> DMatrix<f64> {
        let density_overlap = density_matrix * overlap_matrix;
        let overlap_density = overlap_matrix * density_matrix;
        fock_matrix * density_overlap - overlap_density * fock_matrix
    }

    fn push_history(&mut self, fock_matrix: DMatrix<f64>, error_matrix: DMatrix<f64>) {
        self.history.push_back(DiisEntry {
            fock_matrix,
            error_matrix,
        });
    }

    fn extrapolated_fock_matrix(&self) -> Option<DMatrix<f64>> {
        let history_size = self.history.len();
        if history_size < 2 {
            return None;
        }

        let b_size = history_size + 1;
        let b_values = (0..b_size.pow(2))
            .into_par_iter()
            .map(|index| {
                let i = index % b_size;
                let j = index / b_size;
                match (i == history_size, j == history_size) {
                    (false, false) => self.history[i]
                        .error_matrix
                        .dot(&self.history[j].error_matrix),
                    (true, true) => 0.0,
                    _ => -1.0,
                }
            })
            .collect::<Vec<_>>();
        let b_matrix = DMatrix::from_column_slice(b_size, b_size, &b_values);

        let mut rhs = DVector::zeros(history_size + 1);
        rhs[history_size] = -1.0;

        let coefficients = b_matrix.lu().solve(&rhs)?;
        let nbasis = self.history[0].fock_matrix.nrows();
        let fock_values = (0..nbasis.pow(2))
            .into_par_iter()
            .map(|index| {
                let mu = index % nbasis;
                let nu = index / nbasis;
                (0..history_size)
                    .map(|i| self.history[i].fock_matrix[(mu, nu)] * coefficients[i])
                    .sum()
            })
            .collect::<Vec<_>>();

        Some(DMatrix::from_column_slice(nbasis, nbasis, &fock_values))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_matrix_is_zero_for_commuting_matrices() {
        let fock = DMatrix::from_diagonal(&DVector::from_vec(vec![1.0, 2.0]));
        let density = DMatrix::from_diagonal(&DVector::from_vec(vec![2.0, 0.0]));
        let overlap = DMatrix::identity(2, 2);

        let error = DiisAccelerator::error_matrix(&fock, &density, &overlap);

        assert!(error.norm() < 1e-12);
    }

    #[test]
    fn test_extrapolate_waits_for_two_history_entries() {
        let mut diis = DiisAccelerator::try_new(6).unwrap();
        let fock = DMatrix::identity(2, 2);
        let density = DMatrix::identity(2, 2);
        let overlap = DMatrix::identity(2, 2);

        assert!(diis.extrapolate(&fock, &density, &overlap).is_none());
    }

    #[test]
    fn test_history_discards_oldest_entry() {
        let mut diis = DiisAccelerator::try_new(2).unwrap();
        let error = DMatrix::zeros(1, 1);

        diis.push_history(DMatrix::from_element(1, 1, 1.0), error.clone());
        diis.push_history(DMatrix::from_element(1, 1, 2.0), error.clone());
        diis.push_history(DMatrix::from_element(1, 1, 3.0), error);

        assert_eq!(diis.history.len(), 2);
        assert_eq!(diis.history[0].fock_matrix[(0, 0)], 2.0);
        assert_eq!(diis.history[1].fock_matrix[(0, 0)], 3.0);
    }

    #[test]
    fn test_try_new_rejects_small_history() {
        let result = DiisAccelerator::try_new(1);

        assert!(matches!(result, Err(DiisError::HistoryTooSmall(1))));
    }
}
