use crate::hf::density_guess::random;

use super::DensityGuess;
use nalgebra::DMatrix;
use rand::RngExt;
pub struct Random;

impl DensityGuess for Random {
    fn build_density_guess(
        &self,
        _h_core: &nalgebra::DMatrix<f64>,
        _molecule: &crate::molecules::molecule::Molecule,
        basis: &crate::basis::gaussian::basis::Basis,
    ) -> nalgebra::DMatrix<f64> {
        let nbasis = basis.nbasis();
        let r_iter = rand::rng().random_iter();
        DMatrix::from_iterator(nbasis, nbasis, r_iter)
    }
}
