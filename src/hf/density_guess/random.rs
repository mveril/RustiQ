use super::DensityGuess;
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
    fn build_density_guess(
        &self,
        _h_core: &nalgebra::DMatrix<f64>,
        _molecule: &crate::molecules::molecule::Molecule,
        basis: &crate::basis::gaussian::basis::Basis,
    ) -> nalgebra::DMatrix<f64> {
        let nbasis = basis.nbasis();
        DMatrix::from_iterator(
            nbasis,
            nbasis,
            self.config.random.sample_iter().take(nbasis.pow(2)),
        )
    }
}
