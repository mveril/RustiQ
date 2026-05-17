use super::DensityGuess;
use nalgebra::DMatrix;
use rand::{distributions::Uniform, thread_rng, Rng};
pub struct Random;

impl DensityGuess for Random {
    fn build_density_guess(
        &self,
        _h_core: &nalgebra::DMatrix<f64>,
        _molecule: &crate::molecules::molecule::Molecule,
        basis: &crate::basis::gaussian::basis::Basis,
    ) -> nalgebra::DMatrix<f64> {
        let nbasis = basis.nbasis();
        let r_iter = thread_rng()
            .sample_iter(Uniform::new_inclusive(0.0, 1.0))
            .take(nbasis * nbasis);
        DMatrix::from_iterator(nbasis, nbasis, r_iter)
    }
}
