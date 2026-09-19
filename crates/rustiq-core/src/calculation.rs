//! Frontend-independent HF execution and optional MP2 correlation.
//!
//! [`CalculationBuilder`] validates options, converts coordinates to Bohr, builds
//! the basis and orchestrates HF/MP2. Its prepared inputs can be reused.
//! [`PreparedCalculation::run_hf`] produces an owned [`HfOutcome`]. Its converged
//! variant contains an [`HfSolution<Converged>`] reusable for MP2.
//! Execution errors retain completed HF output. File loading remains with the caller.
//!
//! # Self-consistent field
//!
//! In the AO basis, restricted HF uses the closed-shell density and Fock matrix
//!
//! <div>
//! $$
//! P_{\mu\nu}=2\sum_{i\in\mathrm{occ}}C_{\mu i}C_{\nu i},\qquad
//! F_{\mu\nu}=H^{\mathrm{core}}_{\mu\nu}
//! +\sum_{\lambda\sigma}P_{\lambda\sigma}
//! \left[(\mu\nu\mid\lambda\sigma)
//! -\tfrac12(\mu\sigma\mid\lambda\nu)\right].
//! $$
//! </div>
//!
//! Unrestricted HF has one density per spin, without the factor of two:
//!
//! <div>
//! $$
//! P^s_{\mu\nu}=\sum_{i\in\mathrm{occ}(s)}C^s_{\mu i}C^s_{\nu i},\qquad
//! F^s_{\mu\nu}=H^{\mathrm{core}}_{\mu\nu}
//! +\sum_{\lambda\sigma}(P^\alpha+P^\beta)_{\lambda\sigma}
//! (\mu\nu\mid\lambda\sigma)
//! -\sum_{\lambda\sigma}P^s_{\lambda\sigma}
//! (\mu\sigma\mid\lambda\nu),\quad s\in\{\alpha,\beta\}.
//! $$
//! </div>
//!
//! The Roothaan--Hall equations are $FC=SC\varepsilon$ (one per spin for UHF).
//! Both the change in electronic energy and the norm of the commutator
//! $FPS-SPF$ must be below the configured convergence threshold. The reported
//! total HF energy adds the nuclear repulsion to the electronic energy.
//!
//! The overlap eigendecomposition $S=U\Lambda U^\mathsf{T}$ gives an AO
//! orthogonalizer $X$ satisfying $X^\mathsf{T}SX=I$. With full rank, the code
//! uses symmetric orthogonalization $X=U\Lambda^{-1/2}U^\mathsf{T}$; when
//! eigenvalues fall below the configured relative cutoff, it retains only the
//! corresponding well-conditioned eigenvectors (canonical orthogonalization).
//!
//! # Second-order correlation
//!
//! From converged canonical RHF orbitals, the correlation correction is
//!
//! <div>
//! $$
//! E_\mathrm{MP2}=\sum_{ij\in\mathrm{occ}}\sum_{ab\in\mathrm{virt}}
//! \frac{(ia\mid jb)\bigl[2(ia\mid jb)-(ib\mid ja)\bigr]}
//! {\varepsilon_i+\varepsilon_j-\varepsilon_a-\varepsilon_b}.
//! $$
//! </div>
//!
//! The occupied sums exclude frozen orbitals. UHF MP2 uses separate same-spin
//! and opposite-spin contributions. [`Mp2Result::correlation_energy`] is the
//! correction alone, and [`Mp2Result::electronic_energy`] is the HF electronic
//! energy plus that correction. Add the HF nuclear-repulsion energy to obtain
//! total MP2 energy.

use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

mod setup_error;
pub use setup_error::HfSetupError;
mod solution;
use crate::hf::scf_result::ScfOutcome;
pub use solution::{CalculationExecutionError, Converged, HfOutcome, HfSolution, Unconverged};
mod builder;
mod execution;
mod prepared_calculation;
pub use crate::basis::{Basis, BasisError};
pub use crate::eri::EriError;
use crate::hf::{scf::ScfSetupError, uhf::UhfSetupError};
pub use crate::mp2::Mp2Sector;
pub use crate::{
    hf::{
        density_guess::DensityGuessError,
        diis::DiisError,
        numerical_error::NumericalError,
        orthogonalization::OrthogonalizationInfo,
        scf_energy_details::ScfEnergyDetails,
        scf_iteration::ScfIteration,
        scf_result::{ScfResult, ScfSetupTimings, ScfTimings, SpinDiagnostics},
        scf_setup::ScfSetupStep,
    },
    mp2::{Mp2Error, Mp2Result},
};
pub use builder::CalculationBuilder;
pub use execution::{
    CalculationEvent, CalculationExecution, CalculationResult, HfCalculationResult, Mp2MemoryPlan,
};
pub use prepared_calculation::PreparedCalculation;

use crate::{
    config::{
        HfConfig, HfConfigError, HfMethodResolutionError, MoleculeConfigError, Mp2Config,
        ResolvedHfMethod,
    },
    hf::{
        integrals::{IntegralBuilder, IntegralSetupError},
        scf::ScfCalculation,
        uhf::{alpha_beta_occupied_orbitals, UhfCalculation},
    },
    molecules::molecule::{Molecule, MoleculeError},
    mp2,
};

/// Typed errors with optional input locations, but no source text or renderer.
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
    HfSetup {
        method: ResolvedHfMethod,
        #[source]
        error: HfSetupError,
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
    result: Option<ScfOutcome>,
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
        mut progress: impl FnMut(ScfSetupStep),
    ) -> Result<Self, CalculationError> {
        let method = config.resolve_method(molecule)?;
        let required_occupied_orbitals = match method {
            ResolvedHfMethod::Rhf => molecule.occupied_orbitals(),
            ResolvedHfMethod::Uhf => {
                let occupied = alpha_beta_occupied_orbitals(molecule);
                occupied.alpha.max(occupied.beta)
            }
        };
        let prepared = IntegralBuilder::new(molecule, basis)
            .prepare(
                required_occupied_orbitals,
                config.linear_dependency_threshold.value.into_inner(),
                &mut progress,
            )
            .map_err(|error| CalculationError::HfSetup {
                method,
                span: match &error {
                    IntegralSetupError::Numerical(
                        NumericalError::InsufficientOverlapRank { .. }
                        | NumericalError::InvalidLinearDependencyThreshold { .. },
                    ) => config.linear_dependency_threshold.span,
                    IntegralSetupError::ElectronRepulsion(_) | IntegralSetupError::Numerical(_) => {
                        None
                    }
                },
                error: match error {
                    IntegralSetupError::ElectronRepulsion(error) => error.into(),
                    IntegralSetupError::Numerical(error) => error.into(),
                },
            })?;
        let state = match method {
            ResolvedHfMethod::Rhf => {
                let mut scf = ScfCalculation::new_with_prepared(
                    molecule,
                    basis,
                    config.max_iterations.get(),
                    config.convergence_threshold.into_inner(),
                    config.guess.into_inner(),
                    prepared,
                    progress,
                )
                .map_err(|error| CalculationError::HfSetup {
                    method,
                    span: setup_span(&error, config),
                    error: error.into(),
                })?;
                if config.diis {
                    scf.enable_diis(config.diis_size);
                }
                HfState::Rhf(scf)
            }
            ResolvedHfMethod::Uhf => {
                let mut scf = UhfCalculation::new_with_prepared(
                    molecule,
                    basis,
                    config.max_iterations.get(),
                    config.convergence_threshold.into_inner(),
                    config.guess.into_inner(),
                    prepared,
                    progress,
                )
                .map_err(|error| CalculationError::HfSetup {
                    method,
                    span: match &error {
                        UhfSetupError::Scf(error) => setup_span(error, config),
                        _ => None,
                    },
                    error: error.into(),
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

    pub(crate) fn run(&mut self) -> Result<ScfOutcome, CalculationError> {
        self.run_with_iterations(|_| {})
    }

    pub(crate) fn run_with_iterations(
        &mut self,
        mut observer: impl FnMut(&ScfIteration),
    ) -> Result<ScfOutcome, CalculationError> {
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
        if !matches!(self.result, Some(ScfOutcome::Converged(_))) {
            return Err(CalculationError::HfNotConverged {
                iterations: self
                    .result
                    .as_ref()
                    .map_or(0, |result| result.summary().iterations),
            });
        }
        match &self.state {
            HfState::Rhf(scf) => mp2::rhf_closed_shell_with_memory(
                scf,
                config.frozen_orbitals.into_inner(),
                config.memory_limit.value.resolve().as_u64(),
            ),
            HfState::Uhf(scf) => mp2::uhf_unrestricted_with_memory(
                scf,
                config.frozen_orbitals.into_inner(),
                config.memory_limit.value.resolve().as_u64(),
            ),
        }
        .map_err(|error| CalculationError::Mp2 {
            span: match error {
                Mp2Error::InvalidFrozenOrbitalCount { .. } => config.frozen_orbitals.span,
                Mp2Error::InvalidMemoryLimit
                | Mp2Error::InsufficientMemory { .. }
                | Mp2Error::SizeOverflow => config.memory_limit.span,
                _ => None,
            },
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
