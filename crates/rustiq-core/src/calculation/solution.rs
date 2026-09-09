use std::sync::Arc;

use miette::Diagnostic;
use nalgebra::{DMatrix, DVector};
use thiserror::Error;

use super::{CalculationError, HfCalculationResult, HfState, Mp2Result};
use crate::{
    config::Mp2Config,
    eri::CompactEri,
    mp2::{self, Mp2Input, Mp2SpinInput},
};

#[derive(Debug)]
struct Orbitals {
    coefficients: DMatrix<f64>,
    energies: DVector<f64>,
    occupied: usize,
}

#[derive(Debug)]
struct HfData {
    summary: HfCalculationResult,
    alpha: Orbitals,
    beta: Option<Orbitals>,
    integrals: CompactEri,
}

/// Immutable HF output owning the orbitals and integrals needed for subsequent MP2.
/// Cloning shares scientific data without copying the integral tensor.
#[derive(Debug, Clone)]
pub struct HfSolution(Arc<HfData>);

impl HfSolution {
    pub(super) fn from_state(summary: HfCalculationResult, state: HfState<'_>) -> Self {
        let (alpha, beta, integrals) = match state {
            HfState::Rhf(scf) => (
                Orbitals {
                    coefficients: scf.mo_coefficients,
                    energies: scf.orbital_energies,
                    occupied: scf.occupied_orbitals,
                },
                None,
                scf.two_electron_integrals,
            ),
            HfState::Uhf(scf) => (
                Orbitals {
                    coefficients: scf.mo_coefficients.alpha,
                    energies: scf.orbital_energies.alpha,
                    occupied: scf.occupied_orbitals.alpha,
                },
                Some(Orbitals {
                    coefficients: scf.mo_coefficients.beta,
                    energies: scf.orbital_energies.beta,
                    occupied: scf.occupied_orbitals.beta,
                }),
                scf.two_electron_integrals,
            ),
        };
        Self(Arc::new(HfData {
            summary,
            alpha,
            beta,
            integrals,
        }))
    }

    pub fn summary(&self) -> &HfCalculationResult {
        &self.0.summary
    }

    /// Evaluate MP2 without rerunning HF. A failure retains this HF solution.
    pub fn mp2(&self, config: Mp2Config) -> Result<Mp2Result, CalculationFailure> {
        if !self.summary().scf.converged {
            return Err(self.failure(CalculationError::HfNotConverged {
                iterations: self.summary().scf.iterations,
            }));
        }
        let frozen = config.frozen_orbitals.value;
        let alpha = &self.0.alpha;
        let correlation = if let Some(beta) = &self.0.beta {
            fn spin(o: &Orbitals, frozen: usize) -> Mp2SpinInput<'_> {
                Mp2SpinInput {
                    mo_coefficients: &o.coefficients,
                    orbital_energies: &o.energies,
                    occupied_orbitals: o.occupied,
                    frozen_orbitals: frozen,
                }
            }
            mp2::uhf_correlation_energy(spin(alpha, frozen), spin(beta, frozen), &self.0.integrals)
        } else {
            mp2::correlation_energy(&Mp2Input {
                mo_coefficients: &alpha.coefficients,
                orbital_energies: &alpha.energies,
                occupied_orbitals: alpha.occupied,
                frozen_orbitals: frozen,
                two_electron_integrals: &self.0.integrals,
            })
        };
        let correlation_energy = correlation.map_err(|error| {
            let span = matches!(error, mp2::Mp2Error::InvalidFrozenOrbitalCount { .. })
                .then_some(config.frozen_orbitals.span)
                .flatten();
            self.failure(CalculationError::Mp2 { error, span })
        })?;
        Ok(Mp2Result {
            correlation_energy,
            electronic_energy: self.summary().scf.electronic_energy + correlation_energy,
        })
    }

    fn failure(&self, cause: CalculationError) -> CalculationFailure {
        CalculationFailure {
            cause,
            hf: Some(self.clone()),
        }
    }
}

/// Execution failure with the completed HF stage, if one was produced.
#[derive(Debug, Error, Diagnostic)]
#[error("{cause}")]
#[diagnostic(forward(cause))]
pub struct CalculationFailure {
    #[source]
    cause: CalculationError,
    hf: Option<HfSolution>,
}

impl CalculationFailure {
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

impl From<CalculationError> for CalculationFailure {
    fn from(cause: CalculationError) -> Self {
        Self { cause, hf: None }
    }
}
