use super::Located;
use std::num::NonZeroUsize;

use miette::{Diagnostic, SourceSpan};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    config::validated::{DiisSize, NonNegativeFiniteF64, PositiveFiniteF64},
    molecules::molecule::Molecule,
};

mod density_guess_config;
mod guess_perturbation_config;
mod random_guess_config;

pub use density_guess_config::DensityGuessConfig;
pub use guess_perturbation_config::GuessPerturbationConfig;
pub use random_guess_config::RandomGuessConfig;

#[derive(Debug, Clone)]
pub struct HfConfig {
    pub method: Located<HfMethod>,
    pub max_iterations: NonZeroUsize,
    pub convergence_threshold: PositiveFiniteF64,
    pub linear_dependency_threshold: Located<NonNegativeFiniteF64>,
    pub guess: Located<DensityGuessConfig>,
    pub diis: bool,
    pub diis_size: DiisSize,
}

impl Default for HfConfig {
    fn default() -> Self {
        Self {
            method: HfMethod::default().into(),
            max_iterations: default_max_iter(),
            convergence_threshold: default_conv_threshold(),
            linear_dependency_threshold: default_linear_dependency_threshold().into(),
            guess: DensityGuessConfig::default().into(),
            diis: false,
            diis_size: default_diis_size(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum HfMethod {
    #[default]
    Auto,
    Rhf,
    Uhf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedHfMethod {
    Rhf,
    Uhf,
}

impl std::fmt::Display for ResolvedHfMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rhf => write!(f, "RHF"),
            Self::Uhf => write!(f, "UHF"),
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HfMethodResolutionError {
    #[error(
        "RHF requires a closed-shell singlet: total electrons = {electrons}, multiplicity = {multiplicity}"
    )]
    RhfRequiresClosedShellSinglet { electrons: usize, multiplicity: u8 },
}

/// Error while resolving the HF method from its configuration.
#[derive(Debug, Error, Diagnostic)]
#[error("{error}")]
pub struct HfConfigError {
    #[source]
    pub error: HfMethodResolutionError,
    #[label("requested HF method")]
    pub method_span: Option<SourceSpan>,
}

impl HfConfig {
    pub fn resolve_method(&self, molecule: &Molecule) -> Result<ResolvedHfMethod, HfConfigError> {
        self.method
            .value
            .resolve(molecule)
            .map_err(|error| HfConfigError {
                error,
                method_span: self.method.span,
            })
    }
}

impl HfMethod {
    pub fn resolve(
        &self,
        molecule: &Molecule,
    ) -> Result<ResolvedHfMethod, HfMethodResolutionError> {
        Ok(match self {
            Self::Rhf => {
                if !is_closed_shell_singlet(molecule) {
                    return Err(HfMethodResolutionError::RhfRequiresClosedShellSinglet {
                        electrons: molecule.total_electrons(),
                        multiplicity: molecule.multiplicity().get(),
                    });
                }
                ResolvedHfMethod::Rhf
            }
            Self::Uhf => ResolvedHfMethod::Uhf,
            Self::Auto => {
                if is_closed_shell_singlet(molecule) {
                    ResolvedHfMethod::Rhf
                } else {
                    ResolvedHfMethod::Uhf
                }
            }
        })
    }
}

fn default_conv_threshold() -> PositiveFiniteF64 {
    PositiveFiniteF64::try_new(1e-8).expect("default convergence threshold is positive and finite")
}

fn default_linear_dependency_threshold() -> NonNegativeFiniteF64 {
    NonNegativeFiniteF64::try_new(1e-8)
        .expect("default linear dependency threshold is non-negative and finite")
}

fn default_max_iter() -> NonZeroUsize {
    NonZeroUsize::new(100).expect("default max iterations is non-zero")
}

fn default_diis_size() -> DiisSize {
    DiisSize::try_new(6).expect("default DIIS history size is at least 2")
}

fn is_closed_shell_singlet(molecule: &Molecule) -> bool {
    molecule.multiplicity().get() == 1 && molecule.total_electrons().is_multiple_of(2)
}
