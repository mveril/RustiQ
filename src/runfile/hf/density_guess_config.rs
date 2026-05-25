use serde::{Deserialize, Serialize};

use crate::runfile::hf::{GuessPerturbationConfig, RandomGuessConfig};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "type")]
pub(crate) enum DensityGuessConfig {
    CoreHamiltonian {
        #[serde(default)]
        perturbation: Option<GuessPerturbationConfig>,
    },
    OneElectron {
        #[serde(default)]
        perturbation: Option<GuessPerturbationConfig>,
    },
    Random {
        #[serde(default, flatten)]
        config: RandomGuessConfig,
    },
    RandomSymmetric {
        #[serde(default, flatten)]
        config: RandomGuessConfig,
    },
    Zero,
}

impl Default for DensityGuessConfig {
    fn default() -> Self {
        Self::CoreHamiltonian { perturbation: None }
    }
}
