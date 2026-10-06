use crate::runfile::hf::{GuessPerturbationConfig, RandomGuessConfig};

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(tag = "type")]
pub enum DensityGuessConfig {
    CoreHamiltonian {
        #[serde(skip_serializing_if = "Option::is_none")]
        perturbation: Option<GuessPerturbationConfig>,
    },
    OneElectron {
        #[serde(skip_serializing_if = "Option::is_none")]
        perturbation: Option<GuessPerturbationConfig>,
    },
    Random {
        #[serde(flatten)]
        config: RandomGuessConfig,
    },
    Zero,
}

impl Default for DensityGuessConfig {
    fn default() -> Self {
        Self::CoreHamiltonian { perturbation: None }
    }
}
