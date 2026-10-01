use super::{
    CalculationError, CalculationEvent, CalculationExecution, CalculationExecutionError,
    CalculationResult, HfCalculation, HfCalculationResult, HfOutcome, HfSolution,
};
use crate::hf::scf_result::ScfOutcome;
use crate::{
    basis::Basis,
    config::{HfConfig, Mp2Config, ResolvedHfMethod},
    molecules::molecule::Molecule,
    persistence::EriCache,
};
use std::cell::RefCell;

use super::CalculationRequest;

/// A validated molecule in Bohr and its basis, prepared together by the builder.
///
/// Each execution starts fresh HF state and uses the same immutable inputs.
/// This avoids self-referential SCF storage and allows reuse of the basis.
pub struct PreparedCalculation {
    pub(super) request: CalculationRequest,
    pub(super) molecule: Molecule,
    pub(super) basis: Basis,
    pub(super) basis_name: String,
    pub(super) hf: (HfConfig, ResolvedHfMethod),
    pub(super) mp2: Option<Mp2Config>,
    pub(super) eri_cache: Option<EriCache>,
}

impl PreparedCalculation {
    /// Returns the normalized inputs as requested before scientific resolution.
    pub fn request(&self) -> &CalculationRequest {
        &self.request
    }

    pub fn get_molecule(&self) -> &Molecule {
        &self.molecule
    }

    pub fn get_basis(&self) -> &Basis {
        &self.basis
    }

    pub fn hf_method(&self) -> ResolvedHfMethod {
        self.hf.1
    }

    /// Human-readable label of the loaded basis; this is not its scientific identity.
    /// The resolved basis contents exposed by `get_basis()` are authoritative. Replaying
    /// canonical TOML that uses this label assumes a compatible basis store.
    pub fn basis_name(&self) -> &str {
        &self.basis_name
    }

    /// Resolved HF presentation options with an explicit method and no frontend source spans.
    /// Random seeds resolved during preparation are retained here.
    pub fn hf_config(&self) -> HfConfig {
        let mut config = super::builder::normalized_hf_config(&self.hf.0);
        config.method.value = match self.hf.1 {
            ResolvedHfMethod::Rhf => crate::config::HfMethod::Rhf,
            ResolvedHfMethod::Uhf => crate::config::HfMethod::Uhf,
        };
        config
    }

    /// MP2 options without frontend source spans; automatic memory resolves at execution.
    pub fn mp2_config(&self) -> Option<&Mp2Config> {
        self.request.mp2()
    }
}

impl PreparedCalculation {
    /// Run only HF, retaining orbitals and integrals for subsequent MP2.
    pub fn run_hf(&self) -> Result<HfOutcome, CalculationExecutionError> {
        self.run_hf_with_events(|_| {})
    }

    pub fn run_hf_with_events(
        &self,
        events: impl FnMut(CalculationEvent<'_>),
    ) -> Result<HfOutcome, CalculationExecutionError> {
        let events = RefCell::new(events);
        let (config, method) = &self.hf;
        events.borrow_mut()(CalculationEvent::HfStarted {
            method: *method,
            config,
        });
        let mut calculation = HfCalculation::new_with_progress_and_cache(
            &self.molecule,
            &self.basis,
            config,
            self.eri_cache.as_ref(),
            |step| events.borrow_mut()(CalculationEvent::ScfSetup(step)),
            |event| events.borrow_mut()(CalculationEvent::EriCache(event)),
        )?;
        let outcome = calculation.run_with_iterations(|iteration| {
            events.borrow_mut()(CalculationEvent::ScfIteration(iteration))
        })?;
        let hf = match outcome {
            ScfOutcome::Converged(scf) => HfOutcome::Converged(HfSolution::from_state(
                HfCalculationResult {
                    method: *method,
                    scf,
                },
                calculation.state,
            )),
            ScfOutcome::Unconverged(scf) => HfOutcome::Unconverged(HfSolution::from_state(
                HfCalculationResult {
                    method: *method,
                    scf,
                },
                calculation.state,
            )),
        };
        events.borrow_mut()(CalculationEvent::HfCompleted(&hf));
        Ok(hf)
    }
}

impl CalculationExecution for PreparedCalculation {
    fn execute_with_events(
        &self,
        mut events: impl FnMut(CalculationEvent<'_>),
    ) -> Result<CalculationResult, CalculationExecutionError> {
        let hf = self.run_hf_with_events(&mut events)?;
        let mp2 = self
            .mp2
            .as_ref()
            .map(|config| {
                let converged = match &hf {
                    HfOutcome::Converged(hf) => hf,
                    HfOutcome::Unconverged(_) => {
                        return Err(hf
                            .clone()
                            .execution_error(CalculationError::HfNotConverged {
                                iterations: hf.summary().scf.iterations,
                            }))
                    }
                };
                let result = converged.mp2_with_report(*config, &mut |plan| {
                    events(CalculationEvent::Mp2Planned(plan));
                })?;
                events(CalculationEvent::Mp2Completed {
                    hf: hf.summary(),
                    result: &result,
                });
                Ok::<_, CalculationExecutionError>(result)
            })
            .transpose()?;
        Ok(CalculationResult { hf, mp2 })
    }
}
