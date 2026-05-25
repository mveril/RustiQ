use super::{density_from_fock_like_matrix, DensityGuess};
use crate::basis::gaussian::basis::Basis;
use crate::molecules::molecule::Molecule;
use crate::runfile::random_config::RandomConfig;
use nalgebra::DMatrix;

pub struct RandomSymmetric {
    config: RandomConfig,
}

impl RandomSymmetric {
    pub(crate) fn new(config: RandomConfig) -> Self {
        Self { config }
    }
}

impl Default for RandomSymmetric {
    fn default() -> Self {
        Self::new(RandomConfig::default())
    }
}

impl DensityGuess for RandomSymmetric {
    fn build_density_guess(
        &self,
        _h_core: &DMatrix<f64>,
        molecule: &Molecule,
        basis: &Basis,
    ) -> DMatrix<f64> {
        let nbasis = basis.nbasis();
        let mut rng = self.config.rng();
        let mut random_matrix = DMatrix::zeros(nbasis, nbasis);
        for i in 0..nbasis {
            for j in i..nbasis {
                let value = self.config.sample(&mut rng);
                random_matrix[(i, j)] = value;
                if i != j {
                    random_matrix[(j, i)] = value;
                }
            }
        }

        density_from_fock_like_matrix(&random_matrix, molecule, basis)
    }
}
