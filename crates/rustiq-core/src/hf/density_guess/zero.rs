use crate::hf::reference::HfReference;
use std::convert::Infallible;

use super::{DensityGuess, OrbitalGuess};
use crate::basis::gaussian::basis::Basis;

pub struct Zero;

impl<R: HfReference> DensityGuess<R> for Zero {
    type Error = Infallible;
    fn build_orbital_guess(
        &self,
        _h_core: &nalgebra::DMatrix<f64>,
        _basis: &Basis,
    ) -> Result<OrbitalGuess<R>, Self::Error> {
        Ok(OrbitalGuess::Zero)
    }
}
