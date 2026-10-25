use std::{marker::PhantomData, sync::Arc};

use miette::Diagnostic;
use thiserror::Error;

use super::{CalculationError, HfCalculationResult, HfState, Mp2Result};
use crate::{
    config::{Mp2Config, ResolvedHfMethod, RestrictedMethod, UnrestrictedMethod},
    eri::CompactEri,
    hf::reference::{
        HfReference, MoCoefficients, Occupations, OrbitalEnergies, Restricted, Unrestricted,
    },
    mp2::{self, Mp2Input, Mp2SpinInput},
};

/// An HF solution whose reference is selected at runtime.
#[derive(Debug)]
pub enum AnyHfSolution<State = Converged> {
    Restricted(RhfSolution<State>),
    Unrestricted(UhfSolution<State>),
}

#[derive(Debug, Clone)]
struct Orbitals<R: HfReference> {
    coefficients: MoCoefficients<R>,
    energies: OrbitalEnergies<R>,
    /// Occupation representation selected by the reference profile.
    occupied: Occupations<R>,
}

type RestrictedOrbitals = Orbitals<Restricted>;
type UnrestrictedOrbitals = Orbitals<Unrestricted>;

/// Describes the owned output of a physical SCF method, independently of its driver.
///
/// Coefficients, energies and occupations have independent representations. The
/// descriptor identifies the physical method; sharing orbital representations
/// does not make two descriptors interchangeable. Implementing this trait does
/// not grant HF operations or provide a calculation engine.
///
/// This contract is open to additional descriptors. Construction of solutions is
/// controlled by the library so that convergence cannot be asserted by callers.
pub trait ScfMethod: std::fmt::Debug {
    /// Coefficient representation, including scalar type and channel layout.
    type Coefficients: std::fmt::Debug;
    /// Orbital energy representation; independent of coefficient topology.
    type OrbitalEnergies: std::fmt::Debug;
    /// Occupation representation; independent of coefficient topology.
    type Occupations: std::fmt::Debug;
    /// Method-specific summary, including its numerical diagnostics.
    type Summary: std::fmt::Debug;
    /// Retained scientific data specific to this method.
    type Data: std::fmt::Debug;
}

/// HF method descriptor with a reference-checked physical method identity.
#[derive(Debug)]
pub struct HfModel<R: HfReference>(R::Method);

impl<R: HfReference> ScfMethod for HfModel<R> {
    type Coefficients = MoCoefficients<R>;
    type OrbitalEnergies = OrbitalEnergies<R>;
    type Occupations = Occupations<R>;
    type Summary = HfCalculationResult;
    type Data = CompactEri;
}

impl<R: HfReference> HfModel<R> {
    /// Returns the physical HF method compatible with this reference.
    #[must_use]
    pub fn method(&self) -> R::Method {
        self.0
    }
}

#[derive(Debug)]
struct ScfData<M: ScfMethod> {
    method: M,
    summary: M::Summary,
    coefficients: M::Coefficients,
    energies: M::OrbitalEnergies,
    occupied: M::Occupations,
    data: M::Data,
}

/// Immutable owned SCF output, distinct from the metrics in [`super::ScfResult`].
/// Cloning shares all scientific data, without requiring the descriptor or state
/// marker to implement `Clone`. A descriptor determines physical method identity,
/// independent orbital representations, summary and retained data. The generic
/// contract requires no AO ERIs. MP2 is exposed by the converged HF wrapper only.
///
/// ```compile_fail
/// use rustiq_core::prelude::*;
/// fn invalid<M: ScfMethod>(solution: &ScfSolution<M>) {
///     solution.mp2(Mp2Config::default());
/// }
/// ```
///
/// ```compile_fail
/// use rustiq_core::prelude::*;
/// fn invalid<M: ScfMethod>(solution: ScfSolution<M, Unconverged>) -> ScfSolution<M> {
///     solution
/// }
/// ```
/// Sharing all storage representations does not make physical methods interchangeable:
///
/// ```compile_fail
/// use rustiq_core::prelude::*;
/// fn invalid<M, N>(solution: ScfSolution<M>) -> ScfSolution<N>
/// where
///     M: ScfMethod,
///     N: ScfMethod<Coefficients = M::Coefficients,
///         OrbitalEnergies = M::OrbitalEnergies, Occupations = M::Occupations,
///         Summary = M::Summary, Data = M::Data>,
/// {
///     solution
/// }
/// ```
#[derive(Debug)]
pub struct ScfSolution<M: ScfMethod, State = Converged>(Arc<ScfData<M>>, PhantomData<State>);

impl<M: ScfMethod, State> Clone for ScfSolution<M, State> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0), PhantomData)
    }
}

impl<M: ScfMethod, State> ScfSolution<M, State> {
    /// Borrows the physical method descriptor.
    #[must_use]
    pub fn method(&self) -> &M {
        &self.0.method
    }
    /// Borrows the method-specific summary.
    #[must_use]
    pub fn summary(&self) -> &M::Summary {
        &self.0.summary
    }
    /// Borrows the coefficient representation selected by the method.
    #[must_use]
    pub fn mo_coefficients(&self) -> &M::Coefficients {
        &self.0.coefficients
    }
    /// Borrows orbital energies in the method-specific representation.
    #[must_use]
    pub fn orbital_energies(&self) -> &M::OrbitalEnergies {
        &self.0.energies
    }
    /// Borrows the method-specific occupations.
    #[must_use]
    pub fn occupied_orbitals(&self) -> &M::Occupations {
        &self.0.occupied
    }
    /// Borrows retained scientific data without copying it.
    #[must_use]
    pub fn data(&self) -> &M::Data {
        &self.0.data
    }
}

impl<R: HfReference, State> ScfSolution<HfModel<R>, State> {
    /// Borrows HF SCF metrics without duplicating the summary.
    #[must_use]
    pub fn scf(&self) -> &super::ScfResult {
        &self.summary().scf
    }
}

/// Immutable HF output owning the orbitals and integrals needed for subsequent MP2.
/// Cloning shares scientific data without copying the integral tensor.
/// The reference determines both the allowed method and the orbital component shape.
/// For an unconverged solution, retained orbitals are diagnostic snapshots. They are
/// not guaranteed to diagonalize the final Fock matrix or reproduce the last damped
/// density and its reported energy. Final canonical orbitals are guaranteed only
/// for converged solutions.
///
/// ```compile_fail
/// use rustiq_core::calculation::{HfSolution, Restricted, Unrestricted};
/// fn invalid(hf: HfSolution<Restricted>) -> HfSolution<Unrestricted> { hf }
/// ```
#[derive(Debug)]
pub struct HfSolution<R: HfReference, State = Converged>(ScfSolution<HfModel<R>, State>);

impl<R: HfReference, State> Clone for HfSolution<R, State> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<State> Clone for AnyHfSolution<State> {
    fn clone(&self) -> Self {
        match self {
            Self::Restricted(solution) => Self::Restricted(solution.clone()),
            Self::Unrestricted(solution) => Self::Unrestricted(solution.clone()),
        }
    }
}

/// Restricted closed-shell HF solution, converged by default.
pub type RhfSolution<State = Converged> = HfSolution<Restricted, State>;

/// Unrestricted HF solution, converged by default.
pub type UhfSolution<State = Converged> = HfSolution<Unrestricted, State>;

/// SCF orbitals have passed convergence and final canonicalization.
#[derive(Debug, Clone, Copy)]
pub struct Converged;

/// SCF exhausted its iteration budget without reaching convergence.
#[derive(Debug, Clone, Copy)]
pub struct Unconverged;

/// HF may finish without reaching convergence; inspect the variant before MP2.
///
/// # Offline example
///
/// This complete example uses the bundled H2 singlet and OH doublet fixtures.
/// It inspects RHF/UHF occupations and recovers retained HF output after a
/// deliberately invalid frozen-orbital configuration before retrying MP2.
/// Run it with `cargo run -p rustiq-chem-core --example hf_solution --no-default-features`.
#[doc = concat!("\n```no_run\n", include_str!("../../examples/hf_solution.rs"), "\n```\n")]
#[derive(Debug, Clone)]
pub enum HfOutcome {
    Converged(AnyHfSolution<Converged>),
    Unconverged(AnyHfSolution<Unconverged>),
}

impl HfOutcome {
    /// Borrows the AO ERI tensor, including when HF did not converge.
    #[must_use]
    pub fn ao_eri(&self) -> &CompactEri {
        match self {
            Self::Converged(hf) => hf.ao_eri(),
            Self::Unconverged(hf) => hf.ao_eri(),
        }
    }

    /// Returns the resolved Hartree-Fock method, even when HF did not converge.
    #[must_use]
    pub fn method(&self) -> ResolvedHfMethod {
        self.summary().method
    }

    #[must_use]
    pub fn summary(&self) -> &HfCalculationResult {
        match self {
            Self::Converged(hf) => hf.summary(),
            Self::Unconverged(hf) => hf.summary(),
        }
    }

    /// Borrows the converged solution, if HF reached convergence.
    #[must_use]
    pub fn as_converged(&self) -> Option<&AnyHfSolution<Converged>> {
        match self {
            Self::Converged(hf) => Some(hf),
            Self::Unconverged(_) => None,
        }
    }

    /// Borrows diagnostic HF output when the iteration budget was exhausted.
    /// The returned state does not provide MP2.
    ///
    /// ```compile_fail
    /// use rustiq_core::prelude::*;
    /// fn invalid(outcome: &HfOutcome) {
    ///     if let Some(hf) = outcome.as_unconverged() {
    ///         hf.mp2(Mp2Config::default());
    ///     }
    /// }
    /// ```
    #[must_use]
    pub fn as_unconverged(&self) -> Option<&AnyHfSolution<Unconverged>> {
        match self {
            Self::Converged(_) => None,
            Self::Unconverged(hf) => Some(hf),
        }
    }

    /// Takes the converged solution without cloning its scientific data.
    ///
    /// ```no_run
    /// use rustiq_core::prelude::*;
    /// use rustiq_core::calculation::Mp2Result;
    /// fn correlate(prepared: &PreparedCalculation) -> Result<Mp2Result, CalculationExecutionError> {
    ///     let hf = prepared.run_hf()?.into_converged()?;
    ///     hf.mp2(Mp2Config::default())
    /// }
    /// ```
    ///
    /// # Errors
    /// Returns `HfNotConverged` inside an execution error retaining the complete
    /// unconverged outcome, recoverable through [`CalculationExecutionError::into_hf`].
    pub fn into_converged(self) -> Result<AnyHfSolution<Converged>, CalculationExecutionError> {
        self.try_into()
    }

    pub(super) fn execution_error(self, cause: CalculationError) -> CalculationExecutionError {
        CalculationExecutionError {
            cause,
            hf: Some(self),
        }
    }

    #[must_use]
    pub fn is_converged(&self) -> bool {
        matches!(self, Self::Converged(_))
    }
}

impl<R: HfReference, State> HfSolution<R, State> {
    /// Borrows the method-independent owned output without copying scientific data.
    #[must_use]
    pub fn as_scf(&self) -> &ScfSolution<HfModel<R>, State> {
        &self.0
    }

    /// Takes the method-independent output without copying scientific data.
    #[must_use]
    pub fn into_scf(self) -> ScfSolution<HfModel<R>, State> {
        self.0
    }

    /// Restores the HF wrapper while preserving method identity and convergence state.
    #[must_use]
    pub fn from_scf(solution: ScfSolution<HfModel<R>, State>) -> Self {
        Self(solution)
    }

    /// Borrows the retained AO ERI tensor without copying it.
    #[must_use]
    pub fn ao_eri(&self) -> &CompactEri {
        self.0.data()
    }

    #[must_use]
    pub fn summary(&self) -> &HfCalculationResult {
        self.0.summary()
    }

    /// Returns the method allowed by this reference type.
    #[must_use]
    pub fn method(&self) -> R::Method {
        self.0.method().method()
    }

    /// Borrows shared MO coefficients, or separate alpha and beta coefficients.
    /// Rows follow the AO basis ordering; columns correspond to orbital energies.
    /// Occupied orbitals are the first `n` columns for the relevant occupied count.
    /// Unconverged snapshots need not reproduce the last damped density.
    #[must_use]
    pub fn mo_coefficients(&self) -> &MoCoefficients<R> {
        self.0.mo_coefficients()
    }

    /// Borrows orbital energies in Hartree in the reference-specific representation.
    /// Entries correspond to the coefficient columns. Final-Fock canonicality is
    /// guaranteed only for converged solutions.
    #[must_use]
    pub fn orbital_energies(&self) -> &OrbitalEnergies<R> {
        self.0.orbital_energies()
    }

    /// Borrows the number of occupied spatial orbitals for each reference.
    /// RHF stores one doubly occupied count (`N / 2`); UHF stores separate alpha
    /// and beta counts (`N_alpha` and `N_beta`). Each count selects a prefix of
    /// the corresponding coefficient columns.
    #[must_use]
    pub fn occupied_orbitals(&self) -> &Occupations<R> {
        self.0.occupied_orbitals()
    }

    fn new(
        method: R::Method,
        scf: super::ScfResult,
        orbitals: Orbitals<R>,
        integrals: CompactEri,
    ) -> Self {
        Self(ScfSolution(
            Arc::new(ScfData {
                summary: HfCalculationResult {
                    method: method.into(),
                    scf,
                },
                method: HfModel(method),
                coefficients: orbitals.coefficients,
                energies: orbitals.energies,
                occupied: orbitals.occupied,
                data: integrals,
            }),
            PhantomData,
        ))
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
    /// ```compile_fail
    /// use rustiq_core::prelude::*;
    /// fn invalid(hf: &AnyHfSolution<Unconverged>) {
    ///     if let Some(restricted) = hf.as_restricted() {
    ///         restricted.mp2(Mp2Config::default());
    ///     }
    /// }
    /// ```
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
    /// Returns the original enum unchanged when its reference is unrestricted.
    pub fn into_restricted(self) -> Result<RhfSolution<State>, Self> {
        self.try_into()
    }

    /// Takes the unrestricted solution without cloning its scientific data.
    ///
    /// # Errors
    /// Returns the original enum unchanged when its reference is restricted.
    pub fn into_unrestricted(self) -> Result<UhfSolution<State>, Self> {
        self.try_into()
    }

    pub(super) fn from_state(scf: super::ScfResult, state: HfState<'_>) -> Self {
        match state {
            HfState::Rhf(s) => Self::Restricted(HfSolution::new(
                RestrictedMethod::Rhf,
                scf,
                RestrictedOrbitals {
                    coefficients: s.mo_coefficients,
                    energies: s.orbital_energies,
                    occupied: s.occupied_orbitals,
                },
                s.two_electron_integrals,
            )),
            HfState::Uhf(s) => Self::Unrestricted(HfSolution::new(
                UnrestrictedMethod::Uhf,
                scf,
                UnrestrictedOrbitals {
                    coefficients: s.mo_coefficients,
                    energies: s.orbital_energies,
                    occupied: s.occupied_orbitals,
                },
                s.two_electron_integrals,
            )),
        }
    }
}

/// Extracts converged output, retaining non-converged output in the execution error.
/// Wrapping an unconverged solution does not promote its state:
///
/// ```compile_fail
/// use rustiq_core::prelude::*;
/// fn invalid(hf: AnyHfSolution<Unconverged>) -> AnyHfSolution<Converged> {
///     hf.into()
/// }
/// ```
impl TryFrom<HfOutcome> for AnyHfSolution<Converged> {
    type Error = CalculationExecutionError;

    fn try_from(outcome: HfOutcome) -> Result<Self, Self::Error> {
        match outcome {
            HfOutcome::Converged(hf) => Ok(hf),
            HfOutcome::Unconverged(_) => {
                let iterations = outcome.summary().scf.iterations;
                Err(outcome.execution_error(CalculationError::HfNotConverged { iterations }))
            }
        }
    }
}

/// Extracts an existing restricted solution without changing its convergence state.
impl<State> TryFrom<AnyHfSolution<State>> for RhfSolution<State> {
    type Error = AnyHfSolution<State>;

    fn try_from(hf: AnyHfSolution<State>) -> Result<Self, Self::Error> {
        match hf {
            AnyHfSolution::Restricted(hf) => Ok(hf),
            other @ AnyHfSolution::Unrestricted(_) => Err(other),
        }
    }
}

/// Extracts an existing unrestricted solution without changing its convergence state.
impl<State> TryFrom<AnyHfSolution<State>> for UhfSolution<State> {
    type Error = AnyHfSolution<State>;

    fn try_from(hf: AnyHfSolution<State>) -> Result<Self, Self::Error> {
        match hf {
            other @ AnyHfSolution::Restricted(_) => Err(other),
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

impl From<AnyHfSolution<Converged>> for HfOutcome {
    fn from(hf: AnyHfSolution<Converged>) -> Self {
        Self::Converged(hf)
    }
}

impl From<AnyHfSolution<Unconverged>> for HfOutcome {
    fn from(hf: AnyHfSolution<Unconverged>) -> Self {
        Self::Unconverged(hf)
    }
}

impl AnyHfSolution<Converged> {
    /// Evaluate MP2 without rerunning HF. An error retains this HF solution.
    ///
    /// ```compile_fail
    /// use rustiq_core::{calculation::{HfSolution, Restricted, Unconverged}, config::Mp2Config};
    /// fn invalid(hf: HfSolution<Restricted, Unconverged>) {
    ///     hf.mp2(Mp2Config::default());
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the MP2 configuration, orbital data, memory budget, or numerical result
    /// is invalid.
    pub fn mp2(&self, config: Mp2Config) -> Result<Mp2Result, CalculationExecutionError> {
        self.mp2_with_report(config, &mut |_| {})
    }

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
                            orbitals.occupied_orbitals().alpha,
                        ),
                        spin(
                            &orbitals.mo_coefficients().beta,
                            &orbitals.orbital_energies().beta,
                            orbitals.occupied_orbitals().beta,
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
                            occupied_orbitals: *orbitals.occupied_orbitals(),
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
        CalculationExecutionError {
            cause,
            hf: Some(self.clone().into()),
        }
    }
}

/// Execution error with the completed HF stage, if one was produced.
#[derive(Debug, Error, Diagnostic)]
#[error("{cause}")]
#[diagnostic(forward(cause))]
pub struct CalculationExecutionError {
    #[source]
    cause: CalculationError,
    hf: Option<HfOutcome>,
}

impl CalculationExecutionError {
    #[must_use]
    pub fn cause(&self) -> &CalculationError {
        &self.cause
    }
    #[must_use]
    pub fn hf(&self) -> Option<&HfOutcome> {
        self.hf.as_ref()
    }
    #[must_use]
    pub fn into_hf(self) -> Option<HfOutcome> {
        self.hf
    }
}

impl From<CalculationError> for CalculationExecutionError {
    fn from(cause: CalculationError) -> Self {
        Self { cause, hf: None }
    }
}

impl HfSolution<Restricted, Converged> {
    /// Evaluates MP2 using this converged reference without rerunning HF.
    ///
    /// # Errors
    /// Returns an error retaining HF if the MP2 inputs or computation are invalid.
    pub fn mp2(&self, config: Mp2Config) -> Result<Mp2Result, CalculationExecutionError> {
        AnyHfSolution::from(self.clone()).mp2(config)
    }
}

impl HfSolution<Unrestricted, Converged> {
    /// Evaluates MP2 using this converged reference without rerunning HF.
    ///
    /// # Errors
    /// Returns an error retaining HF if the MP2 inputs or computation are invalid.
    pub fn mp2(&self, config: Mp2Config) -> Result<Mp2Result, CalculationExecutionError> {
        AnyHfSolution::from(self.clone()).mp2(config)
    }
}

#[cfg(test)]
mod clone_tests {
    use super::{AnyHfSolution, HfSolution};
    use crate::hf::reference::Restricted;

    struct NonCloneState;

    #[test]
    fn solutions_are_cloneable_without_cloning_the_state_marker() {
        fn assert_clone<T: Clone>() {}

        assert_clone::<HfSolution<Restricted, NonCloneState>>();
        assert_clone::<AnyHfSolution<NonCloneState>>();
    }
}

#[cfg(test)]
mod extensibility_tests {
    use super::*;

    // Deliberately non-HF storage, with shared coefficients but separate spin counts.
    // No AO ERIs, HF summary, Clone descriptor or shared occupation topology required.
    #[derive(Debug)]
    struct AlternateMethod;
    impl ScfMethod for AlternateMethod {
        type Coefficients = Vec<f64>;
        type OrbitalEnergies = Vec<f64>;
        type Occupations = crate::hf::spin::Spin<usize>;
        type Summary = f64;
        type Data = ();
    }

    #[test]
    fn alternate_output_needs_no_hf_data_and_clones_without_copying() {
        struct State;
        let output: ScfSolution<AlternateMethod, State> = ScfSolution(
            Arc::new(ScfData {
                method: AlternateMethod,
                summary: -0.5,
                coefficients: vec![1.0],
                energies: vec![-0.5],
                occupied: crate::hf::spin::Spin::new(1, 0),
                data: (),
            }),
            PhantomData,
        );
        let cloned = output.clone();
        assert!(Arc::ptr_eq(&output.0, &cloned.0));
        assert_eq!(*cloned.summary(), -0.5);
        assert_eq!(*cloned.occupied_orbitals().alpha(), 1);
        assert_eq!(*cloned.occupied_orbitals().beta(), 0);
    }
}
