use super::{density_from_fock_like_matrix, DensityGuess};
use crate::basis::gaussian::basis::Basis;
use crate::molecules::molecule::Molecule;
use nalgebra::DMatrix;
use rand::{distributions::Uniform, thread_rng, Rng};

pub struct RandomSymmetric;

impl DensityGuess for RandomSymmetric {
    fn build_density_guess(
        &self,
        _h_core: &DMatrix<f64>,
        molecule: &Molecule,
        basis: &Basis,
    ) -> DMatrix<f64> {
        let nbasis = basis.nbasis();
        let r_iter = thread_rng()
            .sample_iter(Uniform::new_inclusive(-1.0, 1.0))
            .take(nbasis * nbasis);
        let random_matrix = DMatrix::from_iterator(nbasis, nbasis, r_iter);
        let symmetric_random_matrix = 0.5 * (&random_matrix + random_matrix.transpose());

        density_from_fock_like_matrix(&symmetric_random_matrix, molecule, basis)
    }
}
