use super::{
    CalculationError, CalculationEvent, CalculationExecution, CalculationExecutionError,
    CalculationResult, HfCalculation, HfCalculationResult, HfOutcome, HfSolution,
};
use crate::eri::CompactEri;
use crate::hf::scf_result::ScfOutcome;
use crate::{
    basis::Basis,
    config::{
        validated::{DiisSize, NonNegativeFiniteF64, PositiveFiniteF64},
        DiisConfig, HfConfig, HfMethod, IntegralConfig, Mp2Config, OrthogonalizationConfig,
        ResolvedHfMethod,
    },
    molecules::{atom::Atom, geometry::Geometry, molecule::Molecule, units::Units},
    persistence::{AoEriArtifact, EriCache, PortableError, RustiQData},
};
use std::{
    cell::RefCell,
    num::{NonZeroU8, NonZeroUsize},
};

use super::artifact_reuse::{ArtifactReuse, CalculationSource};
use super::CalculationRequest;

/// A validated molecule in Bohr and its basis, prepared from configuration or
/// restored from portable scientific data through the same execution interface.
///
/// Each execution starts fresh HF state and uses the same immutable inputs.
/// Available compatible typed artifacts are reused automatically when a source is attached.
/// This avoids self-referential SCF storage and allows reuse of the basis.
pub struct PreparedCalculation {
    pub(super) request: CalculationRequest,
    pub(super) molecule: Molecule,
    pub(super) basis: Basis,
    pub(super) basis_name: String,
    pub(super) hf: (HfConfig, ResolvedHfMethod),
    pub(super) integrals: IntegralConfig,
    pub(super) mp2: Option<Mp2Config>,
    pub(super) eri_cache: Option<EriCache>,
    pub(super) source: CalculationSource,
}

impl PreparedCalculation {
    pub(crate) fn from_portable_context(
        request: CalculationRequest,
        context: &crate::persistence::CalculationContext,
    ) -> Self {
        let atoms = context
            .atoms()
            .map(|(atomic_number, position)| {
                let element = periodic_table::periodic_table()
                    .iter()
                    .find(|element| element.atomic_number == atomic_number)
                    .expect("validated portable context uses a known element");
                Atom::new(
                    element,
                    nalgebra::Point3::new(position[0], position[1], position[2]),
                )
            })
            .collect();
        let geometry = Geometry::new("Restored from RustiQ data".into(), atoms);
        let molecule = Molecule::try_new(
            geometry,
            Units::Bohr,
            context.charge(),
            NonZeroU8::new(context.multiplicity()).expect("validated multiplicity is nonzero"),
        )
        .expect("validated portable context has a valid molecular state");
        let basis = Basis::from_calculation_context(context);

        let diis_size = context
            .diis_size()
            .map(|size| DiisSize::try_new(size).expect("validated DIIS size is valid"));
        let hf = HfConfig {
            method: match context.hf_method() {
                ResolvedHfMethod::Rhf => HfMethod::Rhf,
                ResolvedHfMethod::Uhf => HfMethod::Uhf,
            }
            .into(),
            max_iterations: NonZeroUsize::new(context.max_iterations())
                .expect("validated iteration count is nonzero"),
            convergence_threshold: PositiveFiniteF64::try_new(context.convergence_threshold())
                .expect("validated convergence threshold is positive and finite"),
            guess: context.density_guess().into(),
            diis: DiisConfig {
                enabled: diis_size.is_some(),
                max_history: diis_size
                    .unwrap_or_else(|| DiisSize::try_new(6).expect("default DIIS size is valid"))
                    .into(),
            },
            orthogonalization: OrthogonalizationConfig {
                linear_dependency_threshold: NonNegativeFiniteF64::try_new(
                    context.linear_dependency_threshold(),
                )
                .expect("validated linear dependency threshold is finite and nonnegative")
                .into(),
            },
        };
        let integrals = IntegralConfig {
            schwarz_threshold: context
                .eri_schwarz_threshold()
                .map(|threshold| {
                    PositiveFiniteF64::try_new(threshold)
                        .expect("validated ERI threshold is positive and finite")
                })
                .into(),
        };

        Self {
            basis_name: request.basis_name().to_owned(),
            mp2: request.mp2().copied(),
            request,
            molecule,
            basis,
            hf: (hf, context.hf_method()),
            integrals,
            eri_cache: None,
            source: CalculationSource::Configuration,
        }
    }

    /// Supplies a read-only source of reusable typed scientific artifacts.
    /// Normal execution checks compatibility per artifact, then uses the enabled
    /// local cache or computes missing/incompatible values. Corrupt required
    /// payloads are errors. Newly computed values are retained in memory.
    ///
    /// # Errors
    ///
    /// Returns an error if reusable portable data is incompatible with this calculation.
    pub fn with_reuse_data(mut self, data: RustiQData) -> Result<Self, PortableError> {
        self.source = CalculationSource::Portable(Box::new(ArtifactReuse::new(data, &self)?));
        Ok(self)
    }

    /// Returns the normalized inputs as requested before scientific resolution.
    #[must_use]
    pub fn request(&self) -> &CalculationRequest {
        &self.request
    }

    #[must_use]
    pub fn get_molecule(&self) -> &Molecule {
        &self.molecule
    }

    #[must_use]
    pub fn get_basis(&self) -> &Basis {
        &self.basis
    }

    #[must_use]
    pub fn hf_method(&self) -> ResolvedHfMethod {
        self.hf.1
    }

    /// Human-readable label of the loaded basis; this is not its scientific identity.
    /// The resolved basis contents exposed by `get_basis()` are authoritative. Replaying
    /// canonical TOML that uses this label assumes a compatible basis store.
    #[must_use]
    pub fn basis_name(&self) -> &str {
        &self.basis_name
    }

    /// Resolved HF presentation options with an explicit method and no frontend source spans.
    /// Random seeds resolved during preparation are retained here.
    #[must_use]
    pub fn hf_config(&self) -> HfConfig {
        let mut config = super::builder::normalized_hf_config(&self.hf.0);
        config.method.value = match self.hf.1 {
            ResolvedHfMethod::Rhf => crate::config::HfMethod::Rhf,
            ResolvedHfMethod::Uhf => crate::config::HfMethod::Uhf,
        };
        config
    }

    #[must_use]
    pub fn integral_config(&self) -> IntegralConfig {
        super::builder::normalized_integral_config(&self.integrals)
    }

    /// MP2 options without frontend source spans; automatic memory resolves at execution.
    #[must_use]
    pub fn mp2_config(&self) -> Option<&Mp2Config> {
        self.request.mp2()
    }
}

impl PreparedCalculation {
    /// Run only HF, retaining orbitals and integrals for subsequent MP2.
    ///
    /// # Errors
    ///
    /// Returns an error if HF setup fails or a non-finite numerical value is encountered.
    pub fn run_hf(&self) -> Result<HfOutcome, CalculationExecutionError> {
        self.run_hf_with_events(|_| {})
    }

    /// Runs HF while reporting setup, iteration, and completion events.
    ///
    /// # Errors
    ///
    /// Returns an error if HF setup fails or a non-finite numerical value is encountered.
    pub fn run_hf_with_events(
        &self,
        events: impl FnMut(CalculationEvent<'_>),
    ) -> Result<HfOutcome, CalculationExecutionError> {
        self.run_hf_from_eri(None, events)
    }

    /// Runs HF with an owned AO ERI, bypassing integral computation and the ERI cache.
    ///
    /// # Errors
    ///
    /// Returns an error if the supplied ERI is incompatible or HF preparation or execution fails.
    pub fn run_hf_with_eri(&self, eri: CompactEri) -> Result<HfOutcome, CalculationExecutionError> {
        self.run_hf_from_eri(Some(eri), |_| {})
    }

    fn run_hf_from_eri(
        &self,
        eri: Option<CompactEri>,
        events: impl FnMut(CalculationEvent<'_>),
    ) -> Result<HfOutcome, CalculationExecutionError> {
        let retain_artifact = eri.is_none();
        let events = RefCell::new(events);
        let (config, method) = &self.hf;
        events.borrow_mut()(CalculationEvent::HfStarted {
            method: *method,
            config,
        });
        let eri = match (eri, &self.source) {
            (Some(eri), _) => Some(eri),
            (None, CalculationSource::Portable(reuse)) => {
                let (eri, decision) = reuse
                    .resolve::<AoEriArtifact>(self)
                    .map_err(CalculationError::from)?;
                events.borrow_mut()(CalculationEvent::ArtifactReuse(decision));
                eri
            }
            (None, CalculationSource::Configuration) => None,
        };
        let mut calculation = HfCalculation::new_with_progress_and_cache(
            &self.molecule,
            &self.basis,
            config,
            &self.integrals,
            if eri.is_some() {
                None
            } else {
                self.eri_cache.as_ref()
            },
            eri,
            |step| events.borrow_mut()(CalculationEvent::ScfSetup(step)),
            |event| events.borrow_mut()(CalculationEvent::EriCache(event)),
        )?;
        let outcome = calculation.run_with_iterations(|iteration| {
            events.borrow_mut()(CalculationEvent::ScfIteration(iteration));
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
        if retain_artifact {
            if let CalculationSource::Portable(reuse) = &self.source {
                reuse
                    .remember::<AoEriArtifact>(hf.ao_eri().clone())
                    .map_err(|error| hf.clone().execution_error(CalculationError::from(error)))?;
            }
        }
        events.borrow_mut()(CalculationEvent::HfCompleted(&hf));
        Ok(hf)
    }
}

impl PreparedCalculation {
    /// Executes HF and optional MP2 using the supplied AO ERI without copying it.
    ///
    /// # Errors
    ///
    /// Returns an error if supplied ERIs are incompatible or HF/MP2 execution fails.
    pub fn execute_with_eri(
        &self,
        eri: CompactEri,
    ) -> Result<CalculationResult, CalculationExecutionError> {
        self.execute_from_eri(Some(eri), |_| {})
    }

    fn execute_from_eri(
        &self,
        eri: Option<CompactEri>,
        mut events: impl FnMut(CalculationEvent<'_>),
    ) -> Result<CalculationResult, CalculationExecutionError> {
        let hf = self.run_hf_from_eri(eri, &mut events)?;
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

impl CalculationExecution for PreparedCalculation {
    fn execute_with_events(
        &self,
        events: impl FnMut(CalculationEvent<'_>),
    ) -> Result<CalculationResult, CalculationExecutionError> {
        self.execute_from_eri(None, events)
    }
}
