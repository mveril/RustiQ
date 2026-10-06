use std::num::NonZeroUsize;

use serde::{Deserialize, Serialize};
use toml_spanner::Toml;

use crate::runfile::validated::{DiisSize, NonNegativeFiniteF64, PositiveFiniteF64};
use rustiq_core::molecules::molecule::Molecule;

mod density_guess_config;
mod guess_perturbation_config;
mod random_guess_config;

pub use density_guess_config::DensityGuessConfig;
pub use guess_perturbation_config::GuessPerturbationConfig;
pub use random_guess_config::RandomGuessConfig;

#[derive(Debug, Toml)]
#[toml(Toml, recoverable)]
pub struct HfConfig {
    #[toml(default)]
    pub method: HfMethod,
    #[toml(default = default_max_iter())]
    #[toml(with = crate::runfile::validated::non_zero_usize)]
    pub max_iterations: NonZeroUsize,
    #[toml(default = default_conv_threshold())]
    #[toml(with = crate::runfile::validated::positive_finite_f64)]
    pub convergence_threshold: PositiveFiniteF64,
    #[toml(default, style = Header)]
    pub guess: DensityGuessConfig,
    #[toml(default, style = Header)]
    pub diis: DiisConfig,
    #[toml(default, style = Header)]
    pub orthogonalization: OrthogonalizationConfig,
}

impl Default for HfConfig {
    fn default() -> Self {
        Self {
            method: HfMethod::default(),
            max_iterations: default_max_iter(),
            convergence_threshold: default_conv_threshold(),
            guess: DensityGuessConfig::default(),
            diis: DiisConfig::default(),
            orthogonalization: OrthogonalizationConfig::default(),
        }
    }
}

#[derive(Debug, Toml)]
#[toml(Toml, recoverable)]
pub struct DiisConfig {
    #[toml(default)]
    pub enabled: bool,
    #[toml(default = default_diis_size())]
    #[toml(with = crate::runfile::validated::diis_size)]
    pub max_history: DiisSize,
}

impl Default for DiisConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_history: default_diis_size(),
        }
    }
}

#[derive(Debug, Toml)]
#[toml(Toml, recoverable)]
pub struct OrthogonalizationConfig {
    #[toml(default = default_linear_dependency_threshold())]
    #[toml(with = crate::runfile::validated::non_negative_finite_f64)]
    pub linear_dependency_threshold: NonNegativeFiniteF64,
}

impl Default for OrthogonalizationConfig {
    fn default() -> Self {
        Self {
            linear_dependency_threshold: default_linear_dependency_threshold(),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Toml, PartialEq, Eq)]
#[toml(Toml)]
pub enum HfMethod {
    #[default]
    Auto,
    Rhf,
    Uhf,
}

pub use rustiq_core::config::{HfMethodResolutionError, ResolvedHfMethod};

impl HfMethod {
    pub fn resolve(
        &self,
        molecule: &Molecule,
    ) -> Result<ResolvedHfMethod, HfMethodResolutionError> {
        rustiq_core::config::HfMethod::from(self).resolve(molecule)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "Configuration and portable round trips must preserve literal values and identical execution results exactly"
    )]
    fn hf_defaults_keep_diis_explicitly_disabled() {
        let config: HfConfig = toml_spanner::from_str("").unwrap();

        assert_eq!(config.method, HfMethod::Auto);
        assert_eq!(config.max_iterations.get(), 100);
        assert_eq!(config.convergence_threshold.into_inner(), 1e-8);
        assert!(!config.diis.enabled);
        assert_eq!(config.diis.max_history.into_inner(), 6);
        assert_eq!(
            config
                .orthogonalization
                .linear_dependency_threshold
                .into_inner(),
            1e-8
        );
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "Configuration and portable round trips must preserve literal values and identical execution results exactly"
    )]
    fn nested_diis_and_orthogonalization_deserialize() {
        let config: HfConfig = toml_spanner::from_str(
            r"
            [diis]
            enabled = true
            max_history = 8

            [orthogonalization]
            linear_dependency_threshold = 1e-7
            ",
        )
        .unwrap();

        assert!(config.diis.enabled);
        assert_eq!(config.diis.max_history.into_inner(), 8);
        assert_eq!(
            config
                .orthogonalization
                .linear_dependency_threshold
                .into_inner(),
            1e-7
        );
    }

    #[test]
    fn invalid_diis_history_is_rejected_at_parse_time() {
        assert!(
            toml_spanner::from_str::<HfConfig>("[diis]\nenabled = true\nmax_history = 1").is_err()
        );
    }
}
