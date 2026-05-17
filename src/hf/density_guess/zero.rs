use super::DensityGuess;
use crate::basis::gaussian::basis::Basis;
use crate::molecules::molecule::Molecule;
use nalgebra::DMatrix;

pub struct Zero;

impl DensityGuess for Zero {
    fn build_density_guess(
        &self,
        _h_core: &DMatrix<f64>,
        _molecule: &Molecule,
        basis: &Basis,
    ) -> DMatrix<f64> {
        DMatrix::zeros(basis.nbasis(), basis.nbasis())
    }
}
