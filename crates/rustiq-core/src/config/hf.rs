use super::Located;
use std::num::NonZeroUsize;

use miette::{Diagnostic, SourceSpan};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    config::validated::{DiisSize, NonNegativeFiniteF64, PositiveFiniteF64},
    molecules::Molecule,
};

mod density_guess_config;
mod guess_perturbation_config;
mod random_guess_config;

pub use density_guess_config::DensityGuessConfig;
pub use guess_perturbation_config::GuessPerturbationConfig;
pub use random_guess_config::RandomGuessConfig;

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct HfConfig {
    pub method: Located<HfMethod>,
    pub max_iterations: NonZeroUsize,
    pub convergence_threshold: PositiveFiniteF64,
    pub guess: Located<DensityGuessConfig>,
    pub diis: DiisConfig,
    pub orthogonalization: OrthogonalizationConfig,
}

impl Default for HfConfig {
    fn default() -> Self {
        Self {
            method: HfMethod::default().into(),
            max_iterations: default_max_iter(),
            convergence_threshold: default_conv_threshold(),
            guess: DensityGuessConfig::default().into(),
            diis: DiisConfig::default(),
            orthogonalization: OrthogonalizationConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct DiisConfig {
    pub enabled: bool,
    pub max_history: Located<DiisSize>,
}

impl Default for DiisConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_history: default_diis_size().into(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct OrthogonalizationConfig {
    pub linear_dependency_threshold: Located<NonNegativeFiniteF64>,
}

impl Default for OrthogonalizationConfig {
    fn default() -> Self {
        Self {
            linear_dependency_threshold: default_linear_dependency_threshold().into(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub enum HfMethod {
    #[default]
    Auto,
    Rhf,
    Uhf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResolvedHfMethod {
    Rhf,
    Uhf,
}

/// Methods supported with restricted orbitals.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RestrictedMethod {
    #[default]
    Rhf,
}
/// Methods supported with unrestricted orbitals.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UnrestrictedMethod {
    #[default]
    Uhf,
}
impl From<RestrictedMethod> for ResolvedHfMethod {
    fn from(method: RestrictedMethod) -> Self {
        match method {
            RestrictedMethod::Rhf => Self::Rhf,
        }
    }
}
impl From<UnrestrictedMethod> for ResolvedHfMethod {
    fn from(method: UnrestrictedMethod) -> Self {
        match method {
            UnrestrictedMethod::Uhf => Self::Uhf,
        }
    }
}
impl From<RestrictedMethod> for HfMethod {
    fn from(method: RestrictedMethod) -> Self {
        match method {
            RestrictedMethod::Rhf => Self::Rhf,
        }
    }
}
impl From<UnrestrictedMethod> for HfMethod {
    fn from(method: UnrestrictedMethod) -> Self {
        match method {
            UnrestrictedMethod::Uhf => Self::Uhf,
        }
    }
}
/// A method cannot be represented by the requested HF reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum HfMethodConversionError {
    #[error("method {method:?} is not an explicit restricted HF method")]
    Restricted { method: HfMethod },
    #[error("method {method:?} is not an explicit unrestricted HF method")]
    Unrestricted { method: HfMethod },
}

impl TryFrom<HfMethod> for RestrictedMethod {
    type Error = HfMethodConversionError;
    fn try_from(method: HfMethod) -> Result<Self, Self::Error> {
        match method {
            HfMethod::Rhf => Ok(Self::Rhf),
            HfMethod::Auto | HfMethod::Uhf => Err(HfMethodConversionError::Restricted { method }),
        }
    }
}
impl TryFrom<HfMethod> for UnrestrictedMethod {
    type Error = HfMethodConversionError;
    fn try_from(method: HfMethod) -> Result<Self, Self::Error> {
        match method {
            HfMethod::Uhf => Ok(Self::Uhf),
            HfMethod::Auto | HfMethod::Rhf => Err(HfMethodConversionError::Unrestricted { method }),
        }
    }
}
impl TryFrom<ResolvedHfMethod> for RestrictedMethod {
    type Error = HfMethodConversionError;
    fn try_from(method: ResolvedHfMethod) -> Result<Self, Self::Error> {
        match method {
            ResolvedHfMethod::Rhf => Ok(Self::Rhf),
            ResolvedHfMethod::Uhf => Err(HfMethodConversionError::Restricted {
                method: HfMethod::Uhf,
            }),
        }
    }
}
impl TryFrom<ResolvedHfMethod> for UnrestrictedMethod {
    type Error = HfMethodConversionError;
    fn try_from(method: ResolvedHfMethod) -> Result<Self, Self::Error> {
        match method {
            ResolvedHfMethod::Uhf => Ok(Self::Uhf),
            ResolvedHfMethod::Rhf => Err(HfMethodConversionError::Unrestricted {
                method: HfMethod::Rhf,
            }),
        }
    }
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
#[non_exhaustive]
pub enum HfMethodResolutionError {
    #[error(
        "RHF requires a closed-shell singlet: total electrons = {electrons}, multiplicity = {multiplicity}"
    )]
    RhfRequiresClosedShellSinglet { electrons: usize, multiplicity: u8 },
}

#[derive(Debug, Error, Diagnostic)]
#[error("{error}")]
pub struct HfConfigError {
    #[source]
    pub error: HfMethodResolutionError,
    #[label("requested HF method")]
    pub method_span: Option<SourceSpan>,
}

impl HfConfig {
    /// Resolves the requested HF method for the molecular electron configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if RHF is requested for a molecule that is not a closed-shell singlet.
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
    /// Resolves an explicit or automatic HF method for the molecule.
    ///
    /// # Errors
    ///
    /// Returns an error if RHF is requested for a molecule that is not a closed-shell singlet.
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
