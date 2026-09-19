use std::time::Instant;

use nalgebra::DMatrix;
use thiserror::Error;

use crate::{
    basis::gaussian::basis::Basis,
    eri::{electron_repulsion_ints, CompactEri, EriError},
    molecules::molecule::Molecule,
};

use super::{
    core::core_hamiltonian_ints,
    numerical_error::NumericalError,
    orthogonalization::{ensure_sufficient_rank, orthogonalizer, OrthogonalizationInfo},
    scf_result::ScfSetupTimings,
    scf_setup::ScfSetupStep,
};

/// Integral data shared by restricted and unrestricted SCF calculations.
pub(crate) struct ScfIntegrals {
    pub(crate) overlap: DMatrix<f64>,
    pub(crate) kinetic: DMatrix<f64>,
    pub(crate) nuclear_attraction: DMatrix<f64>,
    pub(crate) electron_repulsion: CompactEri,
}

/// SCF setup data produced before constructing a restricted or unrestricted engine.
pub(crate) struct PreparedScfIntegrals {
    pub(crate) integrals: ScfIntegrals,
    pub(crate) orthogonalizer: DMatrix<f64>,
    pub(crate) orthogonalization: OrthogonalizationInfo,
    pub(crate) timings: ScfSetupTimings,
}

#[derive(Debug, Error)]
pub(crate) enum IntegralSetupError {
    #[error(transparent)]
    ElectronRepulsion(#[from] EriError),
    #[error(transparent)]
    Numerical(#[from] NumericalError),
}

/// Builds the complete set of integrals needed to initialize an SCF engine.
pub(crate) struct IntegralBuilder<'a> {
    molecule: &'a Molecule,
    basis: &'a Basis,
}

impl<'a> IntegralBuilder<'a> {
    pub(crate) fn new(molecule: &'a Molecule, basis: &'a Basis) -> Self {
        Self { molecule, basis }
    }

    pub(crate) fn prepare(
        self,
        occupied_orbitals: usize,
        linear_dependency_threshold: f64,
        mut progress: impl FnMut(ScfSetupStep),
    ) -> Result<PreparedScfIntegrals, IntegralSetupError> {
        let setup_start = Instant::now();
        let mut timings = ScfSetupTimings::default();

        progress(ScfSetupStep::CoreHamiltonian);
        let step_start = Instant::now();
        let (kinetic, nuclear_attraction) = core_hamiltonian_ints(self.molecule, self.basis);
        timings.core_hamiltonian = step_start.elapsed();

        progress(ScfSetupStep::OverlapMatrix);
        let step_start = Instant::now();
        let overlap = self.basis.overlap_ints();
        timings.overlap = step_start.elapsed();
        crate::debug_assert_is_symmetric!(&overlap, 1e-8);

        progress(ScfSetupStep::OverlapOrthogonalizer);
        let step_start = Instant::now();
        let orthogonalization_result =
            orthogonalizer(&overlap, "overlap", linear_dependency_threshold)?;
        let orthogonalization = orthogonalization_result.info;
        ensure_sufficient_rank(orthogonalization, occupied_orbitals)?;
        let orthogonalizer = orthogonalization_result.matrix;
        timings.orthogonalizer = step_start.elapsed();
        progress(ScfSetupStep::OverlapOrthogonalized(orthogonalization));

        progress(ScfSetupStep::ElectronRepulsionIntegrals);
        let step_start = Instant::now();
        let electron_repulsion = electron_repulsion_ints(self.basis)?;
        timings.electron_repulsion_integrals = step_start.elapsed();
        timings.total = setup_start.elapsed();

        Ok(PreparedScfIntegrals {
            integrals: ScfIntegrals {
                overlap,
                kinetic,
                nuclear_attraction,
                electron_repulsion,
            },
            orthogonalizer,
            orthogonalization,
            timings,
        })
    }
}
