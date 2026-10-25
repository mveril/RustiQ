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
///
/// Both supported converged HF references implement this capability:
///
/// ```
/// use rustiq_core::prelude::{Converged, Mp2Calculation, RhfSolution, UhfSolution};
///
/// fn require_mp2<T: Mp2Calculation>() {}
///
/// fn main() {
///     require_mp2::<RhfSolution<Converged>>();
///     require_mp2::<UhfSolution<Converged>>();
/// }
/// ```
///
/// Unconverged references do not implement it:
///
/// ```compile_fail
/// use rustiq_core::prelude::{Mp2Calculation, RhfSolution, Unconverged};
///
/// fn require_mp2<T: Mp2Calculation>() {}
///
/// fn main() {
///     require_mp2::<RhfSolution<Unconverged>>();
/// }
/// ```
///
/// ```compile_fail
/// use rustiq_core::prelude::{Mp2Calculation, UhfSolution, Unconverged};
///
/// fn require_mp2<T: Mp2Calculation>() {}
///
/// fn main() {
///     require_mp2::<UhfSolution<Unconverged>>();
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
