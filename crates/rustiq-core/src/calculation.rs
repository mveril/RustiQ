//! Frontend-independent HF execution and optional MP2 correlation.
//!
//! [`CalculationBuilder`] validates options, converts coordinates to Bohr, builds
//! the basis and orchestrates HF/MP2. Its prepared inputs can be reused.
//! The lower-level [`HfCalculation`] expects a molecule already in Bohr and a
//! basis built from that same geometry. File loading remains with the caller.

use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

mod builder;
mod execution;
mod prepared_calculation;
pub use crate::basis::{Basis, BasisError};
pub use crate::hf::scf::ScfSetupError;
pub use crate::{
    hf::{
        density_guess::DensityGuessError,
        diis::DiisError,
        numerical_error::NumericalError,
        orthogonalization::OrthogonalizationInfo,
        scf_energy_details::ScfEnergyDetails,
        scf_iteration::ScfIteration,
        scf_result::{ScfResult, ScfSetupTimings, ScfTimings},
        scf_setup::ScfSetupStep,
        uhf::UhfSetupError,
    },
    mp2::{Mp2Error, Mp2Result},
};
pub use builder::CalculationBuilder;
pub use execution::{
    CalculationEvent, CalculationExecution, CalculationResult, HfCalculationResult,
};
pub use prepared_calculation::PreparedCalculation;

use crate::{
    config::{
        HfConfig, HfConfigError, HfMethodResolutionError, MoleculeConfigError, Mp2Config,
        ResolvedHfMethod,
    },
    hf::{scf::ScfCalculation, uhf::UhfCalculation},
    molecules::molecule::{Molecule, MoleculeError},
    mp2::{self},
};

/// Typed failures with optional input locations, but no source text or renderer.
#[derive(Debug, Error, Diagnostic)]
pub enum CalculationError {
    #[error(transparent)]
    Basis(#[from] BasisError),
    #[error("{error}")]
    Molecule {
        #[source]
        error: MoleculeError,
        #[label("molecular charge")]
        charge_span: Option<SourceSpan>,
        #[label("spin multiplicity")]
        multiplicity_span: Option<SourceSpan>,
    },
    #[error("{error}")]
    Method {
        #[source]
        error: HfMethodResolutionError,
        #[label("requested HF method")]
        span: Option<SourceSpan>,
    },
    #[error("{error}")]
    RhfSetup {
        #[source]
        error: ScfSetupError<DensityGuessError>,
        #[label("SCF configuration")]
        span: Option<SourceSpan>,
    },
    #[error("{error}")]
    UhfSetup {
        #[source]
        error: UhfSetupError<DensityGuessError>,
        #[label("SCF configuration")]
        span: Option<SourceSpan>,
    },
    #[error(transparent)]
    Numerical(#[from] NumericalError),
    #[error(transparent)]
    Diis(#[from] DiisError),
    #[error("{error}")]
    Mp2 {
        #[source]
        error: Mp2Error,
        #[label("frozen orbital count")]
        span: Option<SourceSpan>,
    },
    #[error(
        "MP2 requires converged HF orbitals, but HF did not converge after {iterations} iterations"
    )]
    HfNotConverged { iterations: usize },
}

impl From<MoleculeConfigError> for CalculationError {
    fn from(error: MoleculeConfigError) -> Self {
        Self::Molecule {
            error: error.error,
            charge_span: error.charge_span,
            multiplicity_span: error.multiplicity_span,
        }
    }
}

impl From<HfConfigError> for CalculationError {
    fn from(error: HfConfigError) -> Self {
        Self::Method {
            error: error.error,
            span: error.method_span,
        }
    }
}

enum HfState<'a> {
    Rhf(ScfCalculation<'a>),
    Uhf(UhfCalculation<'a>),
}

/// A configured RHF or UHF calculation using the canonical scientific engine.
pub(crate) struct HfCalculation<'a> {
    state: HfState<'a>,
    result: Option<ScfResult>,
}

impl<'a> HfCalculation<'a> {
    pub(crate) fn new(
        molecule: &'a Molecule,
        basis: &'a Basis,
        config: &HfConfig,
    ) -> Result<Self, CalculationError> {
        Self::new_with_progress(molecule, basis, config, |_| {})
    }

    pub(crate) fn new_with_progress(
        molecule: &'a Molecule,
        basis: &'a Basis,
        config: &HfConfig,
        progress: impl FnMut(ScfSetupStep),
    ) -> Result<Self, CalculationError> {
        let method = config.resolve_method(molecule)?;
        let state = match method {
            ResolvedHfMethod::Rhf => {
                let mut scf = ScfCalculation::new_with_progress(
                    molecule,
                    basis,
                    config.max_iterations.get(),
                    config.convergence_threshold.into_inner(),
                    config.linear_dependency_threshold.value.into_inner(),
                    config.guess.into_inner(),
                    progress,
                )
                .map_err(|error| CalculationError::RhfSetup {
                    span: setup_span(&error, config),
                    error,
                })?;
                if config.diis {
                    scf.enable_diis(config.diis_size);
                }
                HfState::Rhf(scf)
            }
            ResolvedHfMethod::Uhf => {
                let mut scf = UhfCalculation::new_with_progress(
                    molecule,
                    basis,
                    config.max_iterations.get(),
                    config.convergence_threshold.into_inner(),
                    config.linear_dependency_threshold.value.into_inner(),
                    config.guess.into_inner(),
                    progress,
                )
                .map_err(|error| CalculationError::UhfSetup {
                    span: match &error {
                        UhfSetupError::Scf(error) => setup_span(error, config),
                        _ => None,
                    },
                    error,
                })?;
                if config.diis {
                    scf.enable_diis(config.diis_size.into_inner())?;
                }
                HfState::Uhf(scf)
            }
        };
        Ok(Self {
            state,
            result: None,
        })
    }

    pub(crate) fn method(&self) -> ResolvedHfMethod {
        match &self.state {
            HfState::Rhf(_) => ResolvedHfMethod::Rhf,
            HfState::Uhf(_) => ResolvedHfMethod::Uhf,
        }
    }

    pub(crate) fn run(&mut self) -> Result<ScfResult, CalculationError> {
        self.run_with_iterations(|_| {})
    }

    pub(crate) fn run_with_iterations(
        &mut self,
        mut observer: impl FnMut(&ScfIteration),
    ) -> Result<ScfResult, CalculationError> {
        self.result = None;
        let result = match &mut self.state {
            HfState::Rhf(scf) => scf.run_with_iterations(&mut observer)?,
            HfState::Uhf(scf) => scf.run_with_iterations(&mut observer)?,
        };
        self.result = Some(result.clone());
        Ok(result)
    }

    /// Evaluate MP2 only after this calculation has produced converged orbitals.
    pub(crate) fn mp2(&self, config: &Mp2Config) -> Result<Mp2Result, CalculationError> {
        if !self.result.as_ref().is_some_and(|result| result.converged) {
            return Err(CalculationError::HfNotConverged {
                iterations: self.result.as_ref().map_or(0, |result| result.iterations),
            });
        }
        match &self.state {
            HfState::Rhf(scf) => mp2::rhf_closed_shell(scf, config.frozen_orbitals.into_inner()),
            HfState::Uhf(scf) => mp2::uhf_unrestricted(scf, config.frozen_orbitals.into_inner()),
        }
        .map_err(|error| CalculationError::Mp2 {
            span: matches!(error, Mp2Error::InvalidFrozenOrbitalCount { .. })
                .then_some(config.frozen_orbitals.span)
                .flatten(),
            error,
        })
    }
}

fn setup_span(error: &ScfSetupError<DensityGuessError>, config: &HfConfig) -> Option<SourceSpan> {
    match error {
        ScfSetupError::DensityGuess(DensityGuessError::DistributionCreation(_)) => {
            config.guess.span
        }
        ScfSetupError::Numerical(
            NumericalError::InsufficientOverlapRank { .. }
            | NumericalError::InvalidLinearDependencyThreshold { .. },
        ) => config.linear_dependency_threshold.span,
        _ => None,
    }
}
