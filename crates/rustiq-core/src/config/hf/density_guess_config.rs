use crate::config::hf::{GuessPerturbationConfig, RandomGuessConfig};

#[derive(Debug, Clone, Copy)]
pub enum DensityGuessConfig {
    CoreHamiltonian {
        perturbation: Option<GuessPerturbationConfig>,
    },
    OneElectron {
        perturbation: Option<GuessPerturbationConfig>,
    },
    Random {
        config: RandomGuessConfig,
    },
    Zero,
}

impl Default for DensityGuessConfig {
    fn default() -> Self {
        Self::CoreHamiltonian { perturbation: None }
    }
}
