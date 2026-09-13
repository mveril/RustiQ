use super::orthogonalization::OrthogonalizationInfo;
use super::scf_energy_details::ScfEnergyDetails;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ScfResult {
    pub iterations: usize,
    pub electronic_energy: f64,
    pub nuclear_repulsion_energy: f64,
    pub total_energy: f64,
    pub delta_energy: f64,
    pub residual_norm: f64,
    pub spin: Option<SpinDiagnostics>,
    pub energy_details: ScfEnergyDetails,
    pub orthogonalization: OrthogonalizationInfo,
    pub timings: ScfTimings,
}

/// Spin expectation and contamination of an unrestricted HF determinant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpinDiagnostics {
    pub s_squared: f64,
    pub ideal_s_squared: f64,
    pub spin_contamination: f64,
}

/// The termination of a completed SCF calculation, separate from its metrics.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ScfTermination {
    Converged,
    Unconverged,
}

#[derive(Debug, Clone)]
pub(crate) enum ScfOutcome {
    Converged(ScfResult),
    Unconverged(ScfResult),
}

impl ScfOutcome {
    pub(crate) fn summary(&self) -> &ScfResult {
        match self {
            Self::Converged(result) | Self::Unconverged(result) => result,
        }
    }
}

impl ScfTermination {
    pub(crate) fn with_result(self, result: ScfResult) -> ScfOutcome {
        match self {
            Self::Converged => ScfOutcome::Converged(result),
            Self::Unconverged => ScfOutcome::Unconverged(result),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ScfTimings {
    pub setup: ScfSetupTimings,
    pub iterations: Duration,
    pub final_energy_details: Duration,
    pub total: Duration,
}

#[derive(Debug, Clone, Default)]
pub struct ScfSetupTimings {
    pub core_hamiltonian: Duration,
    pub overlap: Duration,
    pub orthogonalizer: Duration,
    pub electron_repulsion_integrals: Duration,
    pub density_guess: Duration,
    pub initial_orbitals: Duration,
    pub total: Duration,
}
