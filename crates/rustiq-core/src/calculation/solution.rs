use std::{marker::PhantomData, sync::Arc};

use miette::Diagnostic;
use nalgebra::{DMatrix, DVector};
use thiserror::Error;

use super::{CalculationError, HfCalculationResult, HfState, Mp2Result};
use crate::{
    config::{Mp2Config, ResolvedHfMethod},
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
pub struct HfSolution<State>(Arc<HfData>, PhantomData<State>);

/// HF orbitals have passed convergence and final canonicalization.
#[derive(Debug, Clone, Copy)]
pub struct Converged;

/// HF exhausted its iteration budget without reaching convergence.
#[derive(Debug, Clone, Copy)]
pub struct Unconverged;

/// HF may finish without reaching convergence; inspect the variant before MP2.
#[derive(Debug, Clone)]
pub enum HfOutcome {
    Converged(HfSolution<Converged>),
    Unconverged(HfSolution<Unconverged>),
}

impl HfOutcome {
    /// Returns the resolved Hartree-Fock method, even when HF did not converge.
    pub fn method(&self) -> ResolvedHfMethod {
        self.summary().method
    }

    pub fn summary(&self) -> &HfCalculationResult {
        match self {
            Self::Converged(hf) => hf.summary(),
            Self::Unconverged(hf) => hf.summary(),
        }
    }

    pub(super) fn execution_error(self, cause: CalculationError) -> CalculationExecutionError {
        CalculationExecutionError {
            cause,
            hf: Some(self),
        }
    }

    pub fn is_converged(&self) -> bool {
        matches!(self, Self::Converged(_))
    }
}

impl<State> HfSolution<State> {
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
        Self(
            Arc::new(HfData {
                summary,
                alpha,
                beta,
                integrals,
            }),
            PhantomData,
        )
    }

    pub fn summary(&self) -> &HfCalculationResult {
        &self.0.summary
    }

    /// Returns the Hartree-Fock method used to produce this solution.
    pub fn method(&self) -> ResolvedHfMethod {
        self.summary().method
    }
}

impl HfSolution<Converged> {
    /// Evaluate MP2 without rerunning HF. An error retains this HF solution.
    ///
    /// ```compile_fail
    /// use rustiq_core::{calculation::{HfSolution, Unconverged}, config::Mp2Config};
    /// fn invalid(hf: HfSolution<Unconverged>) {
    ///     hf.mp2(Mp2Config::default());
    /// }
    /// ```
    pub fn mp2(&self, config: Mp2Config) -> Result<Mp2Result, CalculationExecutionError> {
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
            hf: Some(HfOutcome::Converged(self.clone())),
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
    hf: Option<HfOutcome>,
}

impl CalculationExecutionError {
    pub fn cause(&self) -> &CalculationError {
        &self.cause
    }
    pub fn hf(&self) -> Option<&HfOutcome> {
        self.hf.as_ref()
    }
    pub fn into_hf(self) -> Option<HfOutcome> {
        self.hf
    }
}

impl From<CalculationError> for CalculationExecutionError {
    fn from(cause: CalculationError) -> Self {
        Self { cause, hf: None }
    }
}
