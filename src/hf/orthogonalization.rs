use nalgebra::DMatrix;

use super::numerical_error::NumericalError;

pub(crate) fn symmetric_orthogonalizer(
    overlap: &DMatrix<f64>,
    label: &'static str,
) -> Result<DMatrix<f64>, NumericalError> {
    let eig = overlap.clone().symmetric_eigen();
    if !eig.eigenvalues.iter().all(|&value| value > 0.0) {
        return Err(NumericalError::IndefiniteMatrix { matrix: label });
    }

    let inv_sqrt_values = eig.eigenvalues.map(|value| 1.0 / value.sqrt());
    Ok(&eig.eigenvectors * DMatrix::from_diagonal(&inv_sqrt_values) * eig.eigenvectors.transpose())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symmetric_orthogonalizer_produces_identity_metric() {
        let overlap = DMatrix::from_row_slice(2, 2, &[2.0, 0.5, 0.5, 1.0]);

        let orthogonalizer = symmetric_orthogonalizer(&overlap, "overlap").unwrap();
        let metric = orthogonalizer.transpose() * overlap * &orthogonalizer;

        assert!(metric.relative_eq(&DMatrix::identity(2, 2), 1e-12, 1e-12));
    }

    #[test]
    fn symmetric_orthogonalizer_rejects_indefinite_matrix() {
        let overlap = DMatrix::from_diagonal(&nalgebra::DVector::from_vec(vec![1.0, -1.0]));

        let error = symmetric_orthogonalizer(&overlap, "overlap").unwrap_err();

        assert!(matches!(
            error,
            NumericalError::IndefiniteMatrix { matrix: "overlap" }
        ));
    }
}
