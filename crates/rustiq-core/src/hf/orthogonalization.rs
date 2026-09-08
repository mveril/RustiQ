use nalgebra::DMatrix;

use super::numerical_error::{ensure_finite_values, NumericalError};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct OrthogonalizationInfo {
    pub basis_dimension: usize,
    pub effective_rank: usize,
    pub discarded_directions: usize,
    pub relative_threshold: f64,
}

#[derive(Debug)]
pub(crate) struct Orthogonalization {
    pub matrix: DMatrix<f64>,
    pub info: OrthogonalizationInfo,
}

pub(crate) fn orthogonalizer(
    overlap: &DMatrix<f64>,
    label: &'static str,
    relative_threshold: f64,
) -> Result<Orthogonalization, NumericalError> {
    if !relative_threshold.is_finite() || relative_threshold < 0.0 {
        return Err(NumericalError::InvalidLinearDependencyThreshold {
            threshold: relative_threshold,
        });
    }

    let eig = overlap.clone().symmetric_eigen();
    ensure_finite_values(&eig.eigenvalues, "overlap eigenvalues")?;

    let largest_eigenvalue = eig
        .eigenvalues
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let cutoff = relative_threshold * largest_eigenvalue;

    if relative_threshold == 0.0 {
        if !eig.eigenvalues.iter().all(|&value| value > 0.0) {
            return Err(NumericalError::IndefiniteMatrix { matrix: label });
        }
    } else if eig.eigenvalues.iter().any(|&value| value < -cutoff) {
        return Err(NumericalError::IndefiniteMatrix { matrix: label });
    }

    let retained: Vec<usize> = eig
        .eigenvalues
        .iter()
        .enumerate()
        .filter_map(|(index, &value)| (value > cutoff).then_some(index))
        .collect();
    let basis_dimension = overlap.nrows();
    let effective_rank = retained.len();
    let info = OrthogonalizationInfo {
        basis_dimension,
        effective_rank,
        discarded_directions: basis_dimension - effective_rank,
        relative_threshold,
    };

    let matrix = if effective_rank == basis_dimension {
        let inv_sqrt_values = eig.eigenvalues.map(|value| 1.0 / value.sqrt());
        &eig.eigenvectors * DMatrix::from_diagonal(&inv_sqrt_values) * eig.eigenvectors.transpose()
    } else {
        let retained_vectors = eig.eigenvectors.select_columns(&retained);
        let inv_sqrt_values = retained
            .iter()
            .map(|&index| 1.0 / eig.eigenvalues[index].sqrt())
            .collect::<Vec<_>>();
        retained_vectors * DMatrix::from_diagonal(&nalgebra::DVector::from_vec(inv_sqrt_values))
    };

    Ok(Orthogonalization { matrix, info })
}

pub(crate) fn ensure_sufficient_rank(
    info: OrthogonalizationInfo,
    occupied_orbitals: usize,
) -> Result<(), NumericalError> {
    if info.effective_rank < occupied_orbitals {
        Err(NumericalError::InsufficientOverlapRank {
            effective_rank: info.effective_rank,
            occupied_orbitals,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_rank_orthogonalizer_produces_identity_metric() {
        let overlap = DMatrix::from_row_slice(2, 2, &[2.0, 0.5, 0.5, 1.0]);

        let result = orthogonalizer(&overlap, "overlap", 1e-8).unwrap();
        let metric = result.matrix.transpose() * overlap * &result.matrix;

        assert_eq!(result.info.effective_rank, 2);
        assert_eq!(result.info.discarded_directions, 0);
        assert!(metric.relative_eq(&DMatrix::identity(2, 2), 1e-12, 1e-12));
    }

    #[test]
    fn canonical_orthogonalizer_discards_small_eigenvalues() {
        let overlap = DMatrix::from_diagonal(&nalgebra::DVector::from_vec(vec![1.0, 1e-14]));

        let result = orthogonalizer(&overlap, "overlap", 1e-8).unwrap();
        let metric = result.matrix.transpose() * overlap * &result.matrix;

        assert_eq!(result.matrix.shape(), (2, 1));
        assert_eq!(result.info.effective_rank, 1);
        assert_eq!(result.info.discarded_directions, 1);
        assert!(metric.relative_eq(&DMatrix::identity(1, 1), 1e-12, 1e-12));
    }

    #[test]
    fn positive_threshold_discards_small_negative_noise() {
        let overlap = DMatrix::from_diagonal(&nalgebra::DVector::from_vec(vec![1.0, -1e-10]));

        let result = orthogonalizer(&overlap, "overlap", 1e-8).unwrap();

        assert_eq!(result.info.effective_rank, 1);
    }

    #[test]
    fn zero_threshold_preserves_strict_positive_definiteness_check() {
        let overlap = DMatrix::from_diagonal(&nalgebra::DVector::from_vec(vec![1.0, 0.0]));

        let error = orthogonalizer(&overlap, "overlap", 0.0).unwrap_err();

        assert!(matches!(
            error,
            NumericalError::IndefiniteMatrix { matrix: "overlap" }
        ));
    }

    #[test]
    fn zero_threshold_keeps_small_positive_eigenvalues() {
        let overlap = DMatrix::from_diagonal(&nalgebra::DVector::from_vec(vec![1.0, 1e-14]));

        let result = orthogonalizer(&overlap, "overlap", 0.0).unwrap();

        assert_eq!(result.matrix.shape(), (2, 2));
        assert_eq!(result.info.effective_rank, 2);
        assert_eq!(result.info.discarded_directions, 0);
    }

    #[test]
    fn canonical_orthogonalizer_rejects_significant_negative_eigenvalue() {
        let overlap = DMatrix::from_diagonal(&nalgebra::DVector::from_vec(vec![1.0, -1e-4]));

        let error = orthogonalizer(&overlap, "overlap", 1e-8).unwrap_err();

        assert!(matches!(
            error,
            NumericalError::IndefiniteMatrix { matrix: "overlap" }
        ));
    }

    #[test]
    fn orthogonalizer_rejects_invalid_threshold() {
        let overlap = DMatrix::identity(1, 1);

        for threshold in [-1.0, f64::NAN, f64::INFINITY] {
            assert!(matches!(
                orthogonalizer(&overlap, "overlap", threshold),
                Err(NumericalError::InvalidLinearDependencyThreshold { .. })
            ));
        }
    }

    #[test]
    fn insufficient_effective_rank_is_rejected() {
        let info = OrthogonalizationInfo {
            effective_rank: 1,
            ..OrthogonalizationInfo::default()
        };

        assert!(matches!(
            ensure_sufficient_rank(info, 2),
            Err(NumericalError::InsufficientOverlapRank {
                effective_rank: 1,
                occupied_orbitals: 2,
            })
        ));
    }
}
