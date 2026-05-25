use serde::{Deserialize, Serialize};

use crate::runfile::hf::RandomGuessConfig;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(tag = "type")]
pub(crate) enum DensityGuessConfig {
    #[default]
    CoreHamiltonian,
    OneElectron,
    Random(RandomGuessConfig),
    RandomSymmetric(RandomGuessConfig),
    Zero,
}
