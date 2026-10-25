use super::{ScfMethod, ScfSolution};
use crate::{
    calculation::{HfCalculationResult, ScfResult},
    eri::CompactEri,
    hf::reference::{HfReference, MoCoefficients, Occupations, OrbitalEnergies},
};

/// HF method descriptor with a reference-checked physical method identity.
/// Adapts [`HfReference`] to the owned-output contract; it does not execute SCF.
#[derive(Debug)]
pub struct HfMethodDescriptor<R: HfReference>(pub(super) R::Method);

impl<R: HfReference> ScfMethod for HfMethodDescriptor<R> {
    type Coefficients = MoCoefficients<R>;
    type OrbitalEnergies = OrbitalEnergies<R>;
    type Occupations = Occupations<R>;
    type Summary = HfCalculationResult;
    type RetainedData = CompactEri;
}

impl<R: HfReference> HfMethodDescriptor<R> {
    /// Returns the physical HF method compatible with this reference.
    #[must_use]
    pub fn method(&self) -> R::Method {
        self.0
    }
}

impl<R: HfReference, State> ScfSolution<HfMethodDescriptor<R>, State> {
    /// Borrows HF SCF metrics without duplicating the summary.
    #[must_use]
    pub fn scf_result(&self) -> &ScfResult {
        &self.summary().scf
    }
}
