use super::{
    AnyHfSolution, CalculationExecutionError, Converged, HfMethodDescriptor, Mp2Calculation,
    ScfSolution,
};
use crate::{
    calculation::{CalculationError, HfCalculationResult, HfOutcome, ScfResult},
    config::Mp2Config,
    eri::CompactEri,
    hf::reference::{
        HfReference, MoCoefficients, Occupations, OrbitalEnergies, Restricted, Unrestricted,
    },
    mp2::{self, Mp2Input, Mp2Result, Mp2SpinInput},
};

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
pub struct HfSolution<R: HfReference, State = Converged>(ScfSolution<HfMethodDescriptor<R>, State>);

impl<R: HfReference, State> Clone for HfSolution<R, State> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

/// Restricted closed-shell HF solution, converged by default.
pub type RhfSolution<State = Converged> = HfSolution<Restricted, State>;

/// Unrestricted HF solution, converged by default.
pub type UhfSolution<State = Converged> = HfSolution<Unrestricted, State>;

impl<R: HfReference, State> HfSolution<R, State> {
    /// Borrows the method-independent owned output via [`AsRef`].
    /// No scientific data is copied.
    #[must_use]
    pub fn as_scf(&self) -> &ScfSolution<HfMethodDescriptor<R>, State> {
        self.as_ref()
    }

    /// Takes the method-independent output via [`From`], preserving the shared allocation.
    #[must_use]
    pub fn into_scf(self) -> ScfSolution<HfMethodDescriptor<R>, State> {
        self.into()
    }

    /// Restores the HF wrapper via [`From`], preserving method identity and convergence state.
    #[must_use]
    pub fn from_scf(solution: ScfSolution<HfMethodDescriptor<R>, State>) -> Self {
        solution.into()
    }

    /// Borrows the retained AO ERI tensor without copying it.
    #[must_use]
    pub fn ao_eri(&self) -> &CompactEri {
        self.0.retained_data()
    }

    #[must_use]
    pub fn summary(&self) -> &HfCalculationResult {
        self.0.summary()
    }

    /// Returns the method allowed by this reference type.
    #[must_use]
    pub fn method(&self) -> R::Method {
        self.0.method_descriptor().method()
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
        self.0.occupations()
    }

    pub(super) fn new(
        method: R::Method,
        scf: ScfResult,
        coefficients: MoCoefficients<R>,
        energies: OrbitalEnergies<R>,
        occupations: Occupations<R>,
        integrals: CompactEri,
    ) -> Self {
        Self(ScfSolution::new(
            HfMethodDescriptor(method),
            HfCalculationResult {
                method: method.into(),
                scf,
            },
            coefficients,
            energies,
            occupations,
            integrals,
        ))
    }
}

/// Transfers HF output into generic output, preserving the shared allocation and state.
///
/// This runnable offline example uses the bundled H2/STO-3G fixture. Hidden setup
/// loads the fixture and runs RHF; the visible code borrows, converts through the
/// runtime enum, extracts the typed output, and evaluates MP2.
///
/// ```
/// # use rustiq_core::prelude::*;
/// # fn inspect<M: ScfMethod, State>(solution: &ScfSolution<M, State>) {
/// #     let _ = solution.mo_coefficients();
/// #     let _ = solution.orbital_energies();
/// #     let _ = solution.occupations();
/// #     let _ = solution.summary();
/// #     let _ = solution.retained_data();
/// # }
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let geometry = Geometry::from_reader(&include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/samples/h2/molecule.xyz"))[..])?;
/// # let basis = BasisFile::from_reader(&include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/sto-3g.json"))[..])?;
/// # let mut molecule = MoleculeConfig::default();
/// # molecule.units = Units::Angstrom;
/// # let prepared = CalculationBuilder::new(&geometry, &basis)
/// #     .with_molecule_config(molecule)
/// #     .prepare()?;
/// # let hf = prepared.run_hf()?.into_converged()?.into_restricted().unwrap_or_else(|_| panic!("H2 singlet should select RHF"));
/// inspect(hf.as_ref());
/// let scf: ScfSolution<HfMethodDescriptor<Restricted>> = hf.into();
/// let runtime: AnyHfSolution = scf.into();
/// let scf = ScfSolution::<HfMethodDescriptor<Restricted>>::try_from(runtime)
///     .unwrap_or_else(|_| panic!("the runtime solution should remain restricted"));
/// let hf = HfSolution::from_scf(scf);
/// assert!(hf.mp2(Mp2Config::default())?.correlation_energy < 0.0);
/// # Ok(())
/// # }
/// ```
///
/// Reference and convergence state cannot change through these conversions:
///
/// ```compile_fail
/// use rustiq_core::prelude::*;
/// fn invalid(solution: UhfSolution)
///     -> ScfSolution<HfMethodDescriptor<Restricted>> {
///     solution.into()
/// }
/// ```
///
/// ```compile_fail
/// use rustiq_core::prelude::*;
/// fn invalid(solution: ScfSolution<HfMethodDescriptor<Restricted>, Unconverged>)
///     -> RhfSolution<Converged> {
///     solution.into()
/// }
/// ```
impl<R: HfReference, State> From<HfSolution<R, State>>
    for ScfSolution<HfMethodDescriptor<R>, State>
{
    fn from(solution: HfSolution<R, State>) -> Self {
        solution.0
    }
}

/// Restores compatible HF output without copying scientific data or changing state.
impl<R: HfReference, State> From<ScfSolution<HfMethodDescriptor<R>, State>>
    for HfSolution<R, State>
{
    fn from(solution: ScfSolution<HfMethodDescriptor<R>, State>) -> Self {
        Self(solution)
    }
}

/// Borrows the generic owned output while keeping the HF specialization explicit.
impl<R: HfReference, State> AsRef<ScfSolution<HfMethodDescriptor<R>, State>>
    for HfSolution<R, State>
{
    fn as_ref(&self) -> &ScfSolution<HfMethodDescriptor<R>, State> {
        &self.0
    }
}

impl HfSolution<Restricted, Converged> {
    /// Evaluates MP2 using this converged reference without rerunning HF.
    ///
    /// # Errors
    /// Returns an error retaining HF if the MP2 inputs or computation are invalid.
    pub fn mp2(&self, config: Mp2Config) -> Result<Mp2Result, CalculationExecutionError> {
        Mp2Calculation::mp2(self, config)
    }

    pub(in crate::calculation) fn mp2_with_report(
        &self,
        config: Mp2Config,
        report: &mut impl FnMut(crate::mp2::Mp2MemoryPlan),
    ) -> Result<Mp2Result, CalculationExecutionError> {
        let orbitals = self.as_scf();
        let correlation = mp2::correlation_energy_with_memory(
            &Mp2Input {
                mo_coefficients: orbitals.mo_coefficients(),
                orbital_energies: orbitals.orbital_energies(),
                occupied_orbitals: *orbitals.occupations(),
                frozen_orbitals: config.frozen_orbitals.value,
                two_electron_integrals: self.ao_eri(),
            },
            config.memory_limit.value.resolve().as_u64(),
            report,
        );
        finish_mp2(
            self.summary().scf.electronic_energy,
            config,
            correlation,
            |cause| {
                let retained: AnyHfSolution = self.clone().into();
                CalculationExecutionError::with_hf(cause, HfOutcome::from(retained))
            },
        )
    }
}

impl HfSolution<Unrestricted, Converged> {
    /// Evaluates MP2 using this converged reference without rerunning HF.
    ///
    /// # Errors
    /// Returns an error retaining HF if the MP2 inputs or computation are invalid.
    pub fn mp2(&self, config: Mp2Config) -> Result<Mp2Result, CalculationExecutionError> {
        Mp2Calculation::mp2(self, config)
    }

    pub(in crate::calculation) fn mp2_with_report(
        &self,
        config: Mp2Config,
        report: &mut impl FnMut(crate::mp2::Mp2MemoryPlan),
    ) -> Result<Mp2Result, CalculationExecutionError> {
        let orbitals = self.as_scf();
        let spin = |coefficients, energies, occupied| Mp2SpinInput {
            mo_coefficients: coefficients,
            orbital_energies: energies,
            occupied_orbitals: occupied,
            frozen_orbitals: config.frozen_orbitals.value,
        };
        let correlation = mp2::uhf_correlation_energy_with_memory(
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
        );
        finish_mp2(
            self.summary().scf.electronic_energy,
            config,
            correlation,
            |cause| {
                let retained: AnyHfSolution = self.clone().into();
                CalculationExecutionError::with_hf(cause, HfOutcome::from(retained))
            },
        )
    }
}

impl Mp2Calculation for HfSolution<Restricted, Converged> {
    fn mp2(&self, config: Mp2Config) -> Result<Mp2Result, CalculationExecutionError> {
        self.mp2_with_report(config, &mut |_| {})
    }
}

impl Mp2Calculation for HfSolution<Unrestricted, Converged> {
    fn mp2(&self, config: Mp2Config) -> Result<Mp2Result, CalculationExecutionError> {
        self.mp2_with_report(config, &mut |_| {})
    }
}

fn finish_mp2(
    electronic_energy: f64,
    config: Mp2Config,
    correlation: Result<f64, mp2::Mp2Error>,
    on_error: impl FnOnce(CalculationError) -> CalculationExecutionError,
) -> Result<Mp2Result, CalculationExecutionError> {
    let correlation_energy = match correlation {
        Ok(energy) => energy,
        Err(error) => {
            let span = match error {
                mp2::Mp2Error::InvalidFrozenOrbitalCount { .. } => config.frozen_orbitals.span,
                mp2::Mp2Error::InvalidMemoryLimit
                | mp2::Mp2Error::InsufficientMemory { .. }
                | mp2::Mp2Error::SizeOverflow => config.memory_limit.span,
                _ => None,
            };
            let cause = CalculationError::Mp2 { error, span };
            return Err(on_error(cause));
        }
    };

    Ok(Mp2Result {
        correlation_energy,
        electronic_energy: electronic_energy + correlation_energy,
    })
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
