use super::DensityGuess;
use nalgebra::DMatrix;
pub struct Random;

impl DensityGuess for Random {
    fn build_density_guess(
        &self,
        _h_core: &nalgebra::DMatrix<f64>,
        _molecule: &crate::molecules::molecule::Molecule,
        basis: &crate::basis::gaussian::basis::Basis,
    ) -> nalgebra::DMatrix<f64> {
        let nbasis = basis.nbasis();
        let r_iter = (0..nbasis.pow(2)).map(|_| rand::random_range(0.0..=1.0));
        DMatrix::from_iterator(nbasis, nbasis, r_iter)
    }
}
