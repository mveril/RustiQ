use super::{
    density_from_fock_like_matrix, perturb_fock_like_matrix, DensityGuess, DensityGuessError,
};
use crate::basis::gaussian::basis::Basis;
use crate::molecules::molecule::Molecule;
use crate::runfile::hf::GuessPerturbationConfig;
use nalgebra::DMatrix;

#[derive(Default)]
pub struct CoreHamiltonian {
    perturbation: Option<GuessPerturbationConfig>,
}

impl CoreHamiltonian {
    pub(crate) fn new(perturbation: Option<GuessPerturbationConfig>) -> Self {
        Self { perturbation }
    }
}

impl DensityGuess for CoreHamiltonian {
    type Error = DensityGuessError;
    fn build_density_guess(
        &self,
        h_core: &DMatrix<f64>,
        molecule: &Molecule,
        basis: &Basis,
    ) -> Result<DMatrix<f64>, Self::Error> {
        let fock_like = perturb_fock_like_matrix(h_core, self.perturbation)?;
        Ok(density_from_fock_like_matrix(&fock_like, molecule, basis)?)
    }
}
