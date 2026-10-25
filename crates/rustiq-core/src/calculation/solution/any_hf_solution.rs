use super::{
    CalculationExecutionError, Converged, HfMethodDescriptor, HfReferenceKind, HfReferenceMismatch,
    HfSolution, RhfSolution, ScfSolution, UhfSolution,
};
use crate::{
    calculation::{CalculationError, HfCalculationResult, HfState, Mp2Result, ScfResult},
    config::{Mp2Config, ResolvedHfMethod, RestrictedMethod, UnrestrictedMethod},
    eri::CompactEri,
    hf::reference::{Restricted, Unrestricted},
    mp2::{self, Mp2Input, Mp2SpinInput},
};

/// An HF solution whose reference is selected at runtime.
///
/// MP2 is available on the typed, converged HF solutions rather than on this runtime enum, so
/// adding another reference does not imply that an MP2 formulation is supported for it.
///
/// ```compile_fail
/// use rustiq_core::{calculation::{AnyHfSolution, Converged}, config::Mp2Config};
/// fn invalid(hf: &AnyHfSolution<Converged>) {
///     hf.mp2(Mp2Config::default());
/// }
/// ```
#[derive(Debug)]
pub enum AnyHfSolution<State = Converged> {
    Restricted(RhfSolution<State>),
    Unrestricted(UhfSolution<State>),
}

impl<State> Clone for AnyHfSolution<State> {
    fn clone(&self) -> Self {
        match self {
            Self::Restricted(solution) => Self::Restricted(solution.clone()),
            Self::Unrestricted(solution) => Self::Unrestricted(solution.clone()),
        }
    }
}

impl<State> AnyHfSolution<State> {
    #[must_use]
    pub fn ao_eri(&self) -> &CompactEri {
        match self {
            Self::Restricted(hf) => hf.ao_eri(),
            Self::Unrestricted(hf) => hf.ao_eri(),
        }
    }

    #[must_use]
    pub fn summary(&self) -> &HfCalculationResult {
        match self {
            Self::Restricted(hf) => hf.summary(),
            Self::Unrestricted(hf) => hf.summary(),
        }
    }

    #[must_use]
    pub fn method(&self) -> ResolvedHfMethod {
        match self {
            Self::Restricted(hf) => hf.method().into(),
            Self::Unrestricted(hf) => hf.method().into(),
        }
    }

    /// Borrows the restricted solution while preserving its convergence state.
    ///
    /// `AnyHfSolution` itself does not expose MP2; call it on a typed, converged HF variant.
    #[must_use]
    pub fn as_restricted(&self) -> Option<&RhfSolution<State>> {
        match self {
            Self::Restricted(hf) => Some(hf),
            Self::Unrestricted(_) => None,
        }
    }

    /// Borrows the unrestricted solution while preserving its convergence state.
    #[must_use]
    pub fn as_unrestricted(&self) -> Option<&UhfSolution<State>> {
        match self {
            Self::Restricted(_) => None,
            Self::Unrestricted(hf) => Some(hf),
        }
    }

    /// Takes the restricted solution without cloning its scientific data.
    ///
    /// # Errors
    /// Returns an error containing the original solution if its reference is unrestricted.
    pub fn into_restricted(self) -> Result<RhfSolution<State>, HfReferenceMismatch<State>> {
        self.try_into()
    }

    /// Takes the unrestricted solution without cloning its scientific data.
    ///
    /// # Errors
    /// Returns an error containing the original solution if its reference is restricted.
    pub fn into_unrestricted(self) -> Result<UhfSolution<State>, HfReferenceMismatch<State>> {
        self.try_into()
    }

    pub(in crate::calculation) fn from_state(scf: ScfResult, state: HfState<'_>) -> Self {
        match state {
            HfState::Rhf(s) => Self::Restricted(HfSolution::new(
                RestrictedMethod::Rhf,
                scf,
                s.mo_coefficients,
                s.orbital_energies,
                s.occupied_orbitals,
                s.two_electron_integrals,
            )),
            HfState::Uhf(s) => Self::Unrestricted(HfSolution::new(
                UnrestrictedMethod::Uhf,
                scf,
                s.mo_coefficients,
                s.orbital_energies,
                s.occupied_orbitals,
                s.two_electron_integrals,
            )),
        }
    }
}

/// Extracts an existing restricted solution without changing its convergence state.
impl<State> TryFrom<AnyHfSolution<State>> for RhfSolution<State> {
    type Error = HfReferenceMismatch<State>;

    fn try_from(hf: AnyHfSolution<State>) -> Result<Self, Self::Error> {
        match hf {
            AnyHfSolution::Restricted(hf) => Ok(hf),
            other @ AnyHfSolution::Unrestricted(_) => Err(HfReferenceMismatch::new(
                HfReferenceKind::Restricted,
                HfReferenceKind::Unrestricted,
                other,
            )),
        }
    }
}

/// Extracts an existing unrestricted solution without changing its convergence state.
impl<State> TryFrom<AnyHfSolution<State>> for UhfSolution<State> {
    type Error = HfReferenceMismatch<State>;

    fn try_from(hf: AnyHfSolution<State>) -> Result<Self, Self::Error> {
        match hf {
            other @ AnyHfSolution::Restricted(_) => Err(HfReferenceMismatch::new(
                HfReferenceKind::Unrestricted,
                HfReferenceKind::Restricted,
                other,
            )),
            AnyHfSolution::Unrestricted(hf) => Ok(hf),
        }
    }
}

impl<State> From<RhfSolution<State>> for AnyHfSolution<State> {
    fn from(hf: RhfSolution<State>) -> Self {
        Self::Restricted(hf)
    }
}

impl<State> From<UhfSolution<State>> for AnyHfSolution<State> {
    fn from(hf: UhfSolution<State>) -> Self {
        Self::Unrestricted(hf)
    }
}

/// Wraps restricted generic HF output in its runtime variant without copying data.
impl<State> From<ScfSolution<HfMethodDescriptor<Restricted>, State>> for AnyHfSolution<State> {
    fn from(solution: ScfSolution<HfMethodDescriptor<Restricted>, State>) -> Self {
        RhfSolution::from(solution).into()
    }
}

/// Wraps unrestricted generic HF output in its runtime variant without copying data.
impl<State> From<ScfSolution<HfMethodDescriptor<Unrestricted>, State>> for AnyHfSolution<State> {
    fn from(solution: ScfSolution<HfMethodDescriptor<Unrestricted>, State>) -> Self {
        UhfSolution::from(solution).into()
    }
}

/// Extracts restricted generic HF output, preserving allocation and convergence state.
/// Returns [`HfReferenceMismatch`] if its reference is unrestricted; the error retains the
/// original solution for recovery.
/// The runnable conversion example on [`HfSolution`] exercises this extraction.
impl<State> TryFrom<AnyHfSolution<State>> for ScfSolution<HfMethodDescriptor<Restricted>, State> {
    type Error = HfReferenceMismatch<State>;

    fn try_from(value: AnyHfSolution<State>) -> Result<Self, Self::Error> {
        RhfSolution::<State>::try_from(value).map(Into::into)
    }
}

/// Extracts unrestricted generic HF output, preserving allocation and convergence state.
/// Returns [`HfReferenceMismatch`] if its reference is restricted; the error retains the
/// original solution for recovery.
impl<State> TryFrom<AnyHfSolution<State>> for ScfSolution<HfMethodDescriptor<Unrestricted>, State> {
    type Error = HfReferenceMismatch<State>;

    fn try_from(value: AnyHfSolution<State>) -> Result<Self, Self::Error> {
        UhfSolution::<State>::try_from(value).map(Into::into)
    }
}

impl AnyHfSolution<Converged> {
    pub(crate) fn mp2_with_report(
        &self,
        config: Mp2Config,
        report: &mut impl FnMut(crate::mp2::Mp2MemoryPlan),
    ) -> Result<Mp2Result, CalculationExecutionError> {
        let frozen = config.frozen_orbitals.value;
        let correlation = match self {
            Self::Unrestricted(hf) => match hf.method() {
                UnrestrictedMethod::Uhf => {
                    let orbitals = hf.as_scf();
                    let spin = |coefficients, energies, occupied| Mp2SpinInput {
                        mo_coefficients: coefficients,
                        orbital_energies: energies,
                        occupied_orbitals: occupied,
                        frozen_orbitals: frozen,
                    };
                    mp2::uhf_correlation_energy_with_memory(
                        spin(
                            &orbitals.mo_coefficients().alpha,
                            &orbitals.orbital_energies().alpha,
                            orbitals.occupations().alpha,
                        ),
                        spin(
                            &orbitals.mo_coefficients().beta,
                            &orbitals.orbital_energies().beta,
                            orbitals.occupations().beta,
                        ),
                        self.ao_eri(),
                        config.memory_limit.value.resolve().as_u64(),
                        report,
                    )
                }
            },
            Self::Restricted(hf) => match hf.method() {
                RestrictedMethod::Rhf => {
                    let orbitals = hf.as_scf();
                    mp2::correlation_energy_with_memory(
                        &Mp2Input {
                            mo_coefficients: orbitals.mo_coefficients(),
                            orbital_energies: orbitals.orbital_energies(),
                            occupied_orbitals: *orbitals.occupations(),
                            frozen_orbitals: frozen,
                            two_electron_integrals: self.ao_eri(),
                        },
                        config.memory_limit.value.resolve().as_u64(),
                        report,
                    )
                }
            },
        };
        let correlation_energy = correlation.map_err(|error| {
            let span = match error {
                mp2::Mp2Error::InvalidFrozenOrbitalCount { .. } => config.frozen_orbitals.span,
                mp2::Mp2Error::InvalidMemoryLimit
                | mp2::Mp2Error::InsufficientMemory { .. }
                | mp2::Mp2Error::SizeOverflow => config.memory_limit.span,
                _ => None,
            };
            self.execution_error(CalculationError::Mp2 { error, span })
        })?;
        Ok(Mp2Result {
            correlation_energy,
            electronic_energy: self.summary().scf.electronic_energy + correlation_energy,
        })
    }

    fn execution_error(&self, cause: CalculationError) -> CalculationExecutionError {
        CalculationExecutionError::with_hf(cause, self.clone().into())
    }
}
