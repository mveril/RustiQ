use super::{
    CalculationError, CalculationEvent, CalculationExecution, CalculationResult, HfCalculation,
    HfCalculationResult,
};
use crate::{
    basis::Basis,
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
    pub(super) hf: (HfConfig, ResolvedHfMethod),
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
    fn execute_with_events(
        &self,
        mut events: impl FnMut(CalculationEvent<'_>),
    ) -> Result<CalculationResult, CalculationError> {
        let (config, method) = &self.hf;
        events(CalculationEvent::HfStarted {
            method: *method,
            config,
        });
        let mut calculation =
            HfCalculation::new_with_progress(&self.molecule, &self.basis, config, |step| {
                events(CalculationEvent::ScfSetup(step))
            })?;
        let hf = HfCalculationResult {
            method: *method,
            scf: calculation.run_with_iterations(|iteration| {
                events(CalculationEvent::ScfIteration(iteration))
            })?,
        };
        events(CalculationEvent::HfCompleted(&hf));
        let mp2 = self
            .mp2
            .as_ref()
            .map(|config| {
                let result = calculation.mp2(config)?;
                events(CalculationEvent::Mp2Completed {
                    hf: &hf,
                    result: &result,
                });
                Ok::<_, CalculationError>(result)
            })
            .transpose()?;
        Ok(CalculationResult { hf, mp2 })
    }
}
