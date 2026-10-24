use super::{symmetric_random_matrix, DensityGuess, DensityGuessError, OrbitalGuess};
use crate::config::hf::RandomGuessConfig;
use crate::hf::reference::HfReference;
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

impl<R: HfReference> DensityGuess<R> for Random {
    type Error = DensityGuessError;
    fn build_orbital_guess(
        &self,
        _h_core: &DMatrix<f64>,
        basis: &crate::basis::gaussian::basis::Basis,
    ) -> Result<OrbitalGuess<R>, Self::Error> {
        let nbasis = basis.nbasis();
        let mut sampler = self.config.random.sample_iter()?;
        Ok(OrbitalGuess::FockLike(R::try_generate_component(|| {
            symmetric_random_matrix(nbasis, &mut sampler)
        })?))
    }
}
