use super::DensityGuess;
use crate::runfile::random_config::RandomConfig;
use nalgebra::DMatrix;

pub struct Random {
    config: RandomConfig,
}

impl Random {
    pub(crate) fn new(config: RandomConfig) -> Self {
        Self { config }
    }
}

impl Default for Random {
    fn default() -> Self {
        Self::new(RandomConfig::default())
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
            self.config.sample_iter().take(nbasis.pow(2)),
        )
    }
}
