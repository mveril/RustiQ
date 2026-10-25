use crate::{calculation::CalculationExecutionError, config::Mp2Config, mp2::Mp2Result};

/// Runs MP2 for a converged reference with an implemented MP2 formulation.
///
/// This capability is implemented only for solution types whose reference has a supported
/// method-specific MP2 treatment. A runtime [`AnyHfSolution`](super::AnyHfSolution) does not
/// implement this trait because containing an HF solution does not by itself guarantee that
/// MP2 is defined for that reference.
///
/// Generic consumers can accept any solution with the capability:
///
/// ```
/// use rustiq_core::prelude::*;
///
/// fn evaluate_mp2<S: Mp2Calculation>(solution: &S) -> Result<
///     rustiq_core::calculation::Mp2Result,
///     rustiq_core::calculation::CalculationExecutionError,
/// > {
///     solution.mp2(Mp2Config::default())
/// }
/// ```
pub trait Mp2Calculation {
    /// Evaluates the MP2 correction without rerunning the reference calculation.
    ///
    /// # Errors
    /// Returns an error retaining the reference solution if the MP2 inputs or computation are
    /// invalid.
    fn mp2(&self, config: Mp2Config) -> Result<Mp2Result, CalculationExecutionError>;
}
