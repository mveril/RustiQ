use super::{density_from_fock_like_matrix, DensityGuess, DensityGuessError};
use crate::basis::gaussian::basis::Basis;
use crate::hf::density_guess::symmetric_random_matrix;
use crate::molecules::molecule::Molecule;
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
    fn build_density_guess(
        &self,
        _h_core: &DMatrix<f64>,
        molecule: &Molecule,
        basis: &Basis,
    ) -> Result<DMatrix<f64>, Self::Error> {
        let nbasis = basis.nbasis();
        let sampler = self.config.random.sample_iter()?;
        let random_matrix = symmetric_random_matrix(nbasis, sampler)?;

        Ok(density_from_fock_like_matrix(
            &random_matrix,
            molecule,
            basis,
        )?)
    }
}
