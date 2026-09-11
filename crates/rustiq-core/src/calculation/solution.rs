use std::sync::Arc;

use miette::Diagnostic;
use nalgebra::{DMatrix, DVector};
use thiserror::Error;

use super::{CalculationError, HfCalculationResult, HfComponent, HfState, Mp2Result, Spin};
use crate::{
    config::Mp2Config,
    eri::CompactEri,
    mp2::{self, Mp2Input, Mp2SpinInput},
};

/// Molecular orbital coefficients (AO rows, MO columns), energies and occupation.
#[derive(Debug, Clone)]
pub struct Orbitals {
    pub coefficients: DMatrix<f64>,
    pub energies: DVector<f64>,
    /// Number of occupied spatial orbitals in this component.
    pub occupied: usize,
}

#[derive(Debug)]
struct HfData {
    summary: HfCalculationResult,
    orbitals: HfComponent<Orbitals>,
    integrals: CompactEri,
}

/// Immutable HF output owning the orbitals and integrals needed for subsequent MP2.
/// Cloning shares scientific data without copying the integral tensor.
#[derive(Debug, Clone)]
pub struct HfSolution(Arc<HfData>);

impl HfSolution {
    pub(super) fn from_state(summary: HfCalculationResult, state: HfState<'_>) -> Self {
        let (orbitals, integrals) = match state {
            HfState::Rhf(scf) => (
                HfComponent::Rhf(Orbitals {
                    coefficients: scf.mo_coefficients,
                    energies: scf.orbital_energies,
                    occupied: scf.occupied_orbitals,
                }),
                scf.two_electron_integrals,
            ),
            HfState::Uhf(scf) => (
                HfComponent::Uhf(Spin {
                    alpha: Orbitals {
                        coefficients: scf.mo_coefficients.alpha,
                        energies: scf.orbital_energies.alpha,
                        occupied: scf.occupied_orbitals.alpha,
                    },
                    beta: Orbitals {
                        coefficients: scf.mo_coefficients.beta,
                        energies: scf.orbital_energies.beta,
                        occupied: scf.occupied_orbitals.beta,
                    },
                }),
                scf.two_electron_integrals,
            ),
        };
        Self(Arc::new(HfData {
            summary,
            orbitals,
            integrals,
        }))
    }

    pub fn summary(&self) -> &HfCalculationResult {
        &self.0.summary
    }

    /// Final SCF orbitals; check `summary().scf.converged` before using them.
    pub fn orbitals(&self) -> &HfComponent<Orbitals> {
        &self.0.orbitals
    }

    /// Evaluate MP2 without rerunning HF. An error retains this HF solution.
    pub fn mp2(&self, config: Mp2Config) -> Result<Mp2Result, CalculationExecutionError> {
        if !self.summary().scf.converged {
            return Err(self.execution_error(CalculationError::HfNotConverged {
                iterations: self.summary().scf.iterations,
            }));
        }
        let frozen = config.frozen_orbitals.value;
        let correlation = match self.orbitals() {
            HfComponent::Uhf(Spin { alpha, beta }) => {
                fn spin(o: &Orbitals, frozen: usize) -> Mp2SpinInput<'_> {
                    Mp2SpinInput {
                        mo_coefficients: &o.coefficients,
                        orbital_energies: &o.energies,
                        occupied_orbitals: o.occupied,
                        frozen_orbitals: frozen,
                    }
                }
                mp2::uhf_correlation_energy(
                    spin(alpha, frozen),
                    spin(beta, frozen),
                    &self.0.integrals,
                )
            }
            HfComponent::Rhf(orbitals) => mp2::correlation_energy(&Mp2Input {
                mo_coefficients: &orbitals.coefficients,
                orbital_energies: &orbitals.energies,
                occupied_orbitals: orbitals.occupied,
                frozen_orbitals: frozen,
                two_electron_integrals: &self.0.integrals,
            }),
        };
        let correlation_energy = correlation.map_err(|error| {
            let span = matches!(error, mp2::Mp2Error::InvalidFrozenOrbitalCount { .. })
                .then_some(config.frozen_orbitals.span)
                .flatten();
            self.execution_error(CalculationError::Mp2 { error, span })
        })?;
        Ok(Mp2Result {
            correlation_energy,
            electronic_energy: self.summary().scf.electronic_energy + correlation_energy,
        })
    }

    fn execution_error(&self, cause: CalculationError) -> CalculationExecutionError {
        CalculationExecutionError {
            cause,
            hf: Some(self.clone()),
        }
    }
}

/// Execution error with the completed HF stage, if one was produced.
#[derive(Debug, Error, Diagnostic)]
#[error("{cause}")]
#[diagnostic(forward(cause))]
pub struct CalculationExecutionError {
    #[source]
    cause: CalculationError,
    hf: Option<HfSolution>,
}

impl CalculationExecutionError {
    pub fn cause(&self) -> &CalculationError {
        &self.cause
    }
    pub fn hf(&self) -> Option<&HfSolution> {
        self.hf.as_ref()
    }
    pub fn into_hf(self) -> Option<HfSolution> {
        self.hf
    }
}

impl From<CalculationError> for CalculationExecutionError {
    fn from(cause: CalculationError) -> Self {
        Self { cause, hf: None }
    }
}
