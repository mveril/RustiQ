use super::{perturb_fock_like_matrices, DensityGuess, DensityGuessError, OrbitalGuess};
use crate::basis::gaussian::basis::Basis;
use crate::config::hf::GuessPerturbationConfig;
use crate::hf::reference::HfReference;
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

impl<R: HfReference> DensityGuess<R> for CoreHamiltonian {
    type Error = DensityGuessError;
    fn build_orbital_guess(
        &self,
        h_core: &DMatrix<f64>,
        _basis: &Basis,
    ) -> Result<OrbitalGuess<R>, Self::Error> {
        match self.perturbation {
            Some(perturbation) => {
                perturb_fock_like_matrices(h_core, perturbation).map_err(Into::into)
            }
            None => Ok(OrbitalGuess::FockLike(R::component_from_spin(
                crate::hf::spin::Spin::duplicate(h_core.clone()),
            ))),
        }
    }
}
