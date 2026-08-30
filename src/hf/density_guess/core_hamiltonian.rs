use super::{perturb_fock_like_matrix, DensityGuess, DensityGuessError, OrbitalGuess};
use crate::basis::gaussian::basis::Basis;
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
    fn build_orbital_guess(
        &self,
        h_core: &DMatrix<f64>,
        _basis: &Basis,
    ) -> Result<OrbitalGuess, Self::Error> {
        let fock_like = perturb_fock_like_matrix(h_core, self.perturbation)?;
        Ok(OrbitalGuess::FockLike(fock_like))
    }
}
