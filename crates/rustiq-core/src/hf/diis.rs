use bounded_vec_deque::BoundedVecDeque;
use nalgebra::{DMatrix, DVector};
use rayon::prelude::*;
use thiserror::Error;

use crate::config::validated::DiisSize;

#[derive(Debug, Error)]
pub enum DiisError {
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
    pub(crate) fn new(max_history: DiisSize) -> Self {
        Self {
            history: BoundedVecDeque::new(max_history.into_inner()),
        }
    }

    pub(crate) fn try_new(max_history: usize) -> Result<Self, DiisError> {
        let max_history =
            DiisSize::try_new(max_history).map_err(|_| DiisError::HistoryTooSmall(max_history))?;
        Ok(Self::new(max_history))
    }

    pub(crate) fn clear(&mut self) {
        self.history.clear();
    }

    pub(crate) fn extrapolate(
        &mut self,
        fock_matrix: &DMatrix<f64>,
        density_matrix: &DMatrix<f64>,
        overlap_matrix: &DMatrix<f64>,
    ) -> Option<DMatrix<f64>> {
        let error_matrix = Self::error_matrix(fock_matrix, density_matrix, overlap_matrix);
        self.extrapolate_with_error(fock_matrix, error_matrix)
    }

    pub(crate) fn extrapolate_with_error(
        &mut self,
        fock_matrix: &DMatrix<f64>,
        error_matrix: DMatrix<f64>,
    ) -> Option<DMatrix<f64>> {
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
        // Nearly dependent residuals can give enormous DIIS weights. Discard
        // older entries until the solve gives bounded, finite coefficients.
        for history_size in (2..=self.history.len()).rev() {
            let start = self.history.len() - history_size;
            let b_size = history_size + 1;
            let b_values = (0..b_size.pow(2))
                .into_par_iter()
                .map(|index| {
                    let i = index % b_size;
                    let j = index / b_size;
                    match (i == history_size, j == history_size) {
                        (false, false) => self.history[start + i]
                            .error_matrix
                            .dot(&self.history[start + j].error_matrix),
                        (true, true) => 0.0,
                        _ => -1.0,
                    }
                })
                .collect::<Vec<_>>();
            let b_matrix = DMatrix::from_column_slice(b_size, b_size, &b_values);

            let mut rhs = DVector::zeros(b_size);
            rhs[history_size] = -1.0;
            let Some(coefficients) = b_matrix.lu().solve(&rhs) else {
                continue;
            };
            if !coefficients
                .iter()
                .all(|coefficient| coefficient.is_finite())
                || coefficients
                    .rows(0, history_size)
                    .iter()
                    .map(|c| c.abs())
                    .sum::<f64>()
                    > 10.0
            {
                continue;
            }

            let (nrows, ncols) = self.history[start].fock_matrix.shape();
            let fock_values = (0..nrows * ncols)
                .into_par_iter()
                .map(|index| {
                    let mu = index % nrows;
                    let nu = index / nrows;
                    (0..history_size)
                        .map(|i| self.history[start + i].fock_matrix[(mu, nu)] * coefficients[i])
                        .sum()
                })
                .collect::<Vec<_>>();
            if fock_values.iter().all(|value: &f64| value.is_finite()) {
                return Some(DMatrix::from_column_slice(nrows, ncols, &fock_values));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_joint_spin_residuals_use_common_weights() {
        let mut diis = DiisAccelerator::try_new(6).unwrap();
        // The spin residuals occupy separate columns: minimizing their squared
        // norm gives weights 4/5 and 1/5, even though each spin alone can cancel.
        let first = DMatrix::from_row_slice(1, 2, &[10.0, 100.0]);
        let second = DMatrix::from_row_slice(1, 2, &[20.0, 200.0]);
        assert!(diis
            .extrapolate_with_error(&first, DMatrix::from_row_slice(1, 2, &[1.0, 0.0]))
            .is_none());
        let result = diis
            .extrapolate_with_error(&second, DMatrix::from_row_slice(1, 2, &[0.0, 2.0]))
            .unwrap();
        approx::assert_abs_diff_eq!(
            result,
            DMatrix::from_row_slice(1, 2, &[12.0, 120.0]),
            epsilon = 1e-12
        );
    }

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
        let mut diis = DiisAccelerator::new(DiisSize::try_new(6).unwrap());
        let fock = DMatrix::identity(2, 2);
        let density = DMatrix::identity(2, 2);
        let overlap = DMatrix::identity(2, 2);

        assert!(diis.extrapolate(&fock, &density, &overlap).is_none());
    }

    #[test]
    fn test_history_discards_oldest_entry() {
        let mut diis = DiisAccelerator::new(DiisSize::try_new(2).unwrap());
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

    #[test]
    fn test_dependent_old_residual_is_dropped() {
        let mut diis = DiisAccelerator::try_new(3).unwrap();
        diis.push_history(
            DMatrix::from_element(1, 1, 100.0),
            DMatrix::from_row_slice(1, 2, &[1.0, 0.0]),
        );
        diis.push_history(
            DMatrix::from_element(1, 1, 2.0),
            DMatrix::from_row_slice(1, 2, &[1.0, 0.0]),
        );
        diis.push_history(
            DMatrix::from_element(1, 1, 3.0),
            DMatrix::from_row_slice(1, 2, &[0.0, 1.0]),
        );

        let result = diis.extrapolated_fock_matrix().unwrap();
        approx::assert_abs_diff_eq!(result[(0, 0)], 2.5, epsilon = 1e-12);
    }

    #[test]
    fn test_large_cancelling_weights_are_rejected() {
        let mut diis = DiisAccelerator::try_new(2).unwrap();
        diis.push_history(
            DMatrix::from_element(1, 1, 1.0),
            DMatrix::from_element(1, 1, 1.0),
        );
        diis.push_history(
            DMatrix::from_element(1, 1, 2.0),
            DMatrix::from_element(1, 1, 1.0001),
        );
        assert!(diis.extrapolated_fock_matrix().is_none());
    }
}
