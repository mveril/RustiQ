use super::{DensityGuessError, EriError, NumericalError};
use crate::hf::{scf::ScfSetupError, uhf::UhfSetupError};
use thiserror::Error;

/// Scientific setup failure shared by RHF and UHF.
#[derive(Debug, Error)]
pub enum HfSetupError {
    #[error(transparent)]
    DensityGuess(#[from] DensityGuessError),
    #[error(transparent)]
    Numerical(#[from] NumericalError),
    #[error(transparent)]
    ElectronRepulsion(#[from] EriError),
}

impl From<ScfSetupError<DensityGuessError>> for HfSetupError {
    fn from(error: ScfSetupError<DensityGuessError>) -> Self {
        match error {
            ScfSetupError::DensityGuess(e) => Self::DensityGuess(e),
            ScfSetupError::Numerical(e) => Self::Numerical(e),
            ScfSetupError::ElectronRepulsion(e) => Self::ElectronRepulsion(e),
        }
    }
}

impl From<UhfSetupError<DensityGuessError>> for HfSetupError {
    fn from(error: UhfSetupError<DensityGuessError>) -> Self {
        match error {
            UhfSetupError::Scf(e) => e.into(),
            UhfSetupError::ElectronRepulsion(e) => Self::ElectronRepulsion(e),
        }
    }
}
