use super::{density_from_fock_like_matrix, DensityGuess};
use crate::basis::gaussian::basis::Basis;
use crate::molecules::molecule::Molecule;
use nalgebra::DMatrix;

pub struct CoreHamiltonian;

impl DensityGuess for CoreHamiltonian {
    fn build_density_guess(
        &self,
        h_core: &DMatrix<f64>,
        molecule: &Molecule,
        basis: &Basis,
    ) -> DMatrix<f64> {
        density_from_fock_like_matrix(h_core, molecule, basis)
    }
}
