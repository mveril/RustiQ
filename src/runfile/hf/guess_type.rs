use serde::{Deserialize, Serialize};

#[derive(Hash, Debug, Default, Serialize, Deserialize)]
pub(crate) enum DensityGuessType {
    #[default]
    CoreHamiltonian,
    OneElectron,
    Random,
    RandomSymmetric,
    Zero,
}
