use super::{symmetric_random_matrix, DensityGuess, DensityGuessError, OrbitalGuess};
use crate::hf::uhf::Spin;
use crate::runfile::hf::RandomGuessConfig;
use nalgebra::DMatrix;

#[derive(Debug, Clone, Copy, Default)]
pub struct Random {
    config: RandomGuessConfig,
}

impl Random {
    pub(crate) fn new(config: RandomGuessConfig) -> Self {
        Self { config }
    }
}

impl DensityGuess for Random {
    type Error = DensityGuessError;
    fn build_orbital_guess(
        &self,
        _h_core: &DMatrix<f64>,
        basis: &crate::basis::gaussian::basis::Basis,
    ) -> Result<OrbitalGuess, Self::Error> {
        let nbasis = basis.nbasis();
        let mut sampler = self.config.random.sample_iter()?;
        Ok(OrbitalGuess::UnrestrictedFockLike(Spin::new(
            symmetric_random_matrix(nbasis, &mut sampler)?,
            symmetric_random_matrix(nbasis, &mut sampler)?,
        )))
    }
}
