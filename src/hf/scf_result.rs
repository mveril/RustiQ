use super::scf_energy_details::ScfEnergyDetails;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ScfResult {
    pub converged: bool,
    pub iterations: usize,
    pub electronic_energy: f64,
    pub nuclear_repulsion_energy: f64,
    pub total_energy: f64,
    pub delta_energy: f64,
    pub residual_norm: f64,
    pub energy_details: ScfEnergyDetails,
    pub timings: ScfTimings,
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
