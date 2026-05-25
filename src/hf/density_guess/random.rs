use super::DensityGuess;
use nalgebra::DMatrix;
use rand::{distr::Uniform, prelude::*, rng};
pub struct Random;

impl DensityGuess for Random {
    fn build_density_guess(
        &self,
        _h_core: &nalgebra::DMatrix<f64>,
        _molecule: &crate::molecules::molecule::Molecule,
        basis: &crate::basis::gaussian::basis::Basis,
    ) -> nalgebra::DMatrix<f64> {
        let nbasis = basis.nbasis();
        let r_iter = Uniform::new(-1f64, 1f64)
            .unwrap()
            .sample_iter(rng())
            .take(nbasis.pow(2));
        DMatrix::from_iterator(nbasis, nbasis, r_iter)
    }
}
