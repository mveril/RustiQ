use self::one_electron::OneElectron;
use self::random::Random;
use crate::basis::gaussian::basis::Basis;
use crate::molecules::molecule::Molecule;
use nalgebra::DMatrix;
use serde::{Deserialize, Serialize};

pub(crate) mod one_electron;
pub(crate) mod random;

pub trait DensityGuess: Send + Sync {
    fn build_density_guess(
        &self,
        h_core: &DMatrix<f64>,
        molecule: &Molecule,
        basis: &Basis,
    ) -> DMatrix<f64>;
}

#[derive(Hash, Debug, Default, Serialize, Deserialize)]
pub enum GuessType {
    #[default]
    OneElectron,
    Random,
}

impl GuessType {
    pub fn get_density_guess(&self) -> Box<dyn DensityGuess> {
        match self {
            Self::OneElectron => Box::new(OneElectron),
            Self::Random => Box::new(Random),
        }
    }
}
