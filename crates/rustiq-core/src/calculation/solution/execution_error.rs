use miette::Diagnostic;
use thiserror::Error;

use super::HfOutcome;
use crate::calculation::CalculationError;

/// Execution error with the completed HF stage, if one was produced.
#[derive(Debug, Error, Diagnostic)]
#[error("{cause}")]
#[diagnostic(forward(cause))]
pub struct CalculationExecutionError {
    #[source]
    cause: CalculationError,
    hf: Option<HfOutcome>,
}

impl CalculationExecutionError {
    pub(super) fn with_hf(cause: CalculationError, hf: HfOutcome) -> Self {
        Self {
            cause,
            hf: Some(hf),
        }
    }

    #[must_use]
    pub fn cause(&self) -> &CalculationError {
        &self.cause
    }
    #[must_use]
    pub fn hf(&self) -> Option<&HfOutcome> {
        self.hf.as_ref()
    }
    #[must_use]
    pub fn into_hf(self) -> Option<HfOutcome> {
        self.hf
    }
}

impl From<CalculationError> for CalculationExecutionError {
    fn from(cause: CalculationError) -> Self {
        Self { cause, hf: None }
    }
}
