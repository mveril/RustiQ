use super::{AnyHfSolution, CalculationExecutionError, Converged, Unconverged};
use crate::{
    calculation::{CalculationError, HfCalculationResult},
    config::ResolvedHfMethod,
    eri::CompactEri,
};

/// HF may finish without reaching convergence; inspect the variant before MP2.
///
/// # Offline example
///
/// This complete example uses the bundled H2 singlet and OH doublet fixtures.
/// It inspects RHF/UHF occupations and recovers retained HF output after a
/// deliberately invalid frozen-orbital configuration before retrying MP2.
/// Run it with `cargo run -p rustiq-chem-core --example hf_solution --no-default-features`.
#[doc = concat!("\n```no_run\n", include_str!("../../../examples/hf_solution.rs"), "\n```\n")]
#[derive(Debug, Clone)]
pub enum HfOutcome {
    Converged(AnyHfSolution<Converged>),
    Unconverged(AnyHfSolution<Unconverged>),
}

impl HfOutcome {
    /// Borrows the AO ERI tensor, including when HF did not converge.
    #[must_use]
    pub fn ao_eri(&self) -> &CompactEri {
        match self {
            Self::Converged(hf) => hf.ao_eri(),
            Self::Unconverged(hf) => hf.ao_eri(),
        }
    }

    /// Returns the resolved Hartree-Fock method, even when HF did not converge.
    #[must_use]
    pub fn method(&self) -> ResolvedHfMethod {
        self.summary().method
    }

    #[must_use]
    pub fn summary(&self) -> &HfCalculationResult {
        match self {
            Self::Converged(hf) => hf.summary(),
            Self::Unconverged(hf) => hf.summary(),
        }
    }

    /// Borrows the converged solution, if HF reached convergence.
    #[must_use]
    pub fn as_converged(&self) -> Option<&AnyHfSolution<Converged>> {
        match self {
            Self::Converged(hf) => Some(hf),
            Self::Unconverged(_) => None,
        }
    }

    /// Borrows diagnostic HF output when the iteration budget was exhausted.
    /// The returned state does not provide MP2.
    ///
    /// ```compile_fail
    /// use rustiq_core::prelude::*;
    /// fn invalid(outcome: &HfOutcome) {
    ///     if let Some(hf) = outcome.as_unconverged() {
    ///         hf.mp2(Mp2Config::default());
    ///     }
    /// }
    /// ```
    #[must_use]
    pub fn as_unconverged(&self) -> Option<&AnyHfSolution<Unconverged>> {
        match self {
            Self::Converged(_) => None,
            Self::Unconverged(hf) => Some(hf),
        }
    }

    /// Takes the converged solution without cloning its scientific data.
    ///
    /// ```no_run
    /// use rustiq_core::prelude::*;
    /// use rustiq_core::calculation::Mp2Result;
    /// fn correlate(prepared: &PreparedCalculation) -> Result<Mp2Result, CalculationExecutionError> {
    ///     let hf = prepared.run_hf()?.into_converged()?;
    ///     hf.mp2(Mp2Config::default())
    /// }
    /// ```
    ///
    /// # Errors
    /// Returns `HfNotConverged` inside an execution error retaining the complete
    /// unconverged outcome, recoverable through [`CalculationExecutionError::into_hf`].
    pub fn into_converged(self) -> Result<AnyHfSolution<Converged>, CalculationExecutionError> {
        self.try_into()
    }

    pub(in crate::calculation) fn execution_error(
        self,
        cause: CalculationError,
    ) -> CalculationExecutionError {
        CalculationExecutionError::with_hf(cause, self)
    }

    #[must_use]
    pub fn is_converged(&self) -> bool {
        matches!(self, Self::Converged(_))
    }
}

/// Extracts converged output, retaining non-converged output in the execution error.
/// Wrapping an unconverged solution does not promote its state:
///
/// ```compile_fail
/// use rustiq_core::prelude::*;
/// fn invalid(hf: AnyHfSolution<Unconverged>) -> AnyHfSolution<Converged> {
///     hf.into()
/// }
/// ```
impl TryFrom<HfOutcome> for AnyHfSolution<Converged> {
    type Error = CalculationExecutionError;

    fn try_from(outcome: HfOutcome) -> Result<Self, Self::Error> {
        match outcome {
            HfOutcome::Converged(hf) => Ok(hf),
            HfOutcome::Unconverged(_) => {
                let iterations = outcome.summary().scf.iterations;
                Err(outcome.execution_error(CalculationError::HfNotConverged { iterations }))
            }
        }
    }
}

impl From<AnyHfSolution<Converged>> for HfOutcome {
    fn from(hf: AnyHfSolution<Converged>) -> Self {
        Self::Converged(hf)
    }
}

impl From<AnyHfSolution<Unconverged>> for HfOutcome {
    fn from(hf: AnyHfSolution<Unconverged>) -> Self {
        Self::Unconverged(hf)
    }
}
