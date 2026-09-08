use super::{
    CalculationError, CalculationExecution, CalculationObserver, CalculationResult, HfCalculation,
    HfCalculationResult,
};
use crate::{
    basis::gaussian::basis::Basis,
    config::{HfConfig, Mp2Config, ResolvedHfMethod},
    molecules::molecule::Molecule,
};

/// A validated molecule in Bohr and its basis, prepared together by the builder.
///
/// Each execution starts fresh HF state and uses the same immutable inputs.
/// This avoids self-referential SCF storage and allows reuse of the basis.
pub struct PreparedCalculation {
    pub(super) molecule: Molecule,
    pub(super) basis: Basis,
    pub(super) hf: Option<(HfConfig, ResolvedHfMethod)>,
    pub(super) mp2: Option<Mp2Config>,
}

impl PreparedCalculation {
    pub fn get_molecule(&self) -> &Molecule {
        &self.molecule
    }

    pub fn get_basis(&self) -> &Basis {
        &self.basis
    }
}

impl CalculationExecution for PreparedCalculation {
    fn execute_with_observer(
        &self,
        observer: &mut impl CalculationObserver,
    ) -> Result<CalculationResult, CalculationError> {
        let Some((config, method)) = &self.hf else {
            return Ok(CalculationResult::default());
        };
        observer.on_hf_start(*method, config);
        let mut calculation =
            HfCalculation::new_with_progress(&self.molecule, &self.basis, config, |step| {
                observer.on_scf_setup_step(step)
            })?;
        let hf = HfCalculationResult {
            method: *method,
            scf: calculation.run_with_observer(observer)?,
        };
        observer.on_hf_complete(&hf);
        let mp2 = self
            .mp2
            .as_ref()
            .map(|config| {
                let result = calculation.mp2(config)?;
                observer.on_mp2_complete(&hf, &result);
                Ok::<_, CalculationError>(result)
            })
            .transpose()?;
        Ok(CalculationResult { hf: Some(hf), mp2 })
    }
}
