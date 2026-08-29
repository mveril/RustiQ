use nalgebra::DVector;
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum NumericalError {
    #[error("{matrix} matrix is not positive definite")]
    IndefiniteMatrix { matrix: &'static str },
    #[error("{values} contain a non-finite value")]
    NonFiniteValues { values: &'static str },
    #[error("{value} is not finite")]
    NonFiniteScalar { value: &'static str },
}

pub(crate) fn ensure_finite_values(
    values: &DVector<f64>,
    label: &'static str,
) -> Result<(), NumericalError> {
    if values.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(NumericalError::NonFiniteValues { values: label })
    }
}

pub(crate) fn ensure_finite_value(value: f64, label: &'static str) -> Result<(), NumericalError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(NumericalError::NonFiniteScalar { value: label })
    }
}
