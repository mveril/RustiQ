use super::{density_from_fock_like_matrix, DensityGuess};
use crate::basis::gaussian::basis::Basis;
use crate::molecules::molecule::Molecule;
use nalgebra::DMatrix;
use rand::RngExt;

pub struct RandomSymmetric;

impl DensityGuess for RandomSymmetric {
    fn build_density_guess(
        &self,
        _h_core: &DMatrix<f64>,
        molecule: &Molecule,
        basis: &Basis,
    ) -> DMatrix<f64> {
        let nbasis = basis.nbasis();
        let r_iter = rand::rng().random_iter();
        let random_matrix = DMatrix::from_iterator(nbasis, nbasis, r_iter);
        let symmetric_random_matrix = 0.5 * (&random_matrix + random_matrix.transpose());
        density_from_fock_like_matrix(&symmetric_random_matrix, molecule, basis)
    }
}
