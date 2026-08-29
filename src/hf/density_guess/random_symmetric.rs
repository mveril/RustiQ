use super::{symmetric_random_matrix, DensityGuess, DensityGuessError, OrbitalGuess};
use crate::basis::gaussian::basis::Basis;
use crate::runfile::hf::RandomGuessConfig;
use nalgebra::DMatrix;

#[derive(Debug, Clone, Copy, Default)]
pub struct RandomSymmetric {
    config: RandomGuessConfig,
}

impl RandomSymmetric {
    pub(crate) fn new(config: RandomGuessConfig) -> Self {
        Self { config }
    }
}

impl DensityGuess for RandomSymmetric {
    type Error = DensityGuessError;
    fn build_orbital_guess(
        &self,
        _h_core: &DMatrix<f64>,
        basis: &Basis,
    ) -> Result<OrbitalGuess, Self::Error> {
        let nbasis = basis.nbasis();
        let sampler = self.config.random.sample_iter()?;
        let random_matrix = symmetric_random_matrix(nbasis, sampler)?;

        Ok(OrbitalGuess::FockLike(random_matrix))
    }
}
