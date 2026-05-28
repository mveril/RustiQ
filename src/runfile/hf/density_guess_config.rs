use toml_spanner::{helper::flatten_any, Toml};

use crate::runfile::hf::{GuessPerturbationConfig, RandomGuessConfig};

#[derive(Debug, Clone, Copy, Toml)]
#[toml(Toml, tag = "type")]
pub(crate) enum DensityGuessConfig {
    CoreHamiltonian {
        #[toml(default)]
        perturbation: Option<GuessPerturbationConfig>,
    },
    OneElectron {
        #[toml(default)]
        perturbation: Option<GuessPerturbationConfig>,
    },
    Random {
        #[toml(default, flatten, with = flatten_any)]
        config: RandomGuessConfig,
    },
    RandomSymmetric {
        #[toml(default, flatten, with = flatten_any)]
        config: RandomGuessConfig,
    },
    Zero,
}

impl Default for DensityGuessConfig {
    fn default() -> Self {
        Self::CoreHamiltonian { perturbation: None }
    }
}
