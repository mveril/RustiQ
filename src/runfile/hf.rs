use serde::{Deserialize, Serialize};

use crate::hf::density_guess::GuessType as DensityGuessType;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct HfConfig {
    #[serde(default = "default_max_iter")]
    pub max_iterations: usize,
    #[serde(default = "default_conv_threshold")]
    pub convergence_threshold: f64,
    #[serde(default)]
    pub density_guess: DensityGuessType,
}

fn default_conv_threshold() -> f64 {
    1e-8
}

fn default_max_iter() -> usize {
    100
}
