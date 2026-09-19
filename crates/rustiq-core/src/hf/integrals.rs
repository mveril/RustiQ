use std::time::Instant;

use nalgebra::DMatrix;

use crate::{
    basis::gaussian::basis::Basis,
    eri::{electron_repulsion_ints, CompactEri, EriError},
    molecules::molecule::Molecule,
};

use super::{core::core_hamiltonian_ints, scf_result::ScfSetupTimings, scf_setup::ScfSetupStep};

/// Integral data shared by restricted and unrestricted SCF calculations.
pub(crate) struct ScfIntegrals {
    pub(crate) overlap: DMatrix<f64>,
    pub(crate) kinetic: DMatrix<f64>,
    pub(crate) nuclear_attraction: DMatrix<f64>,
    pub(crate) electron_repulsion: CompactEri,
    pub(crate) timings: ScfSetupTimings,
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

    pub(crate) fn build(
        self,
        mut progress: impl FnMut(ScfSetupStep),
    ) -> Result<ScfIntegrals, EriError> {
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

        let step_start = Instant::now();
        let electron_repulsion = electron_repulsion_ints(self.basis)?;
        timings.electron_repulsion_integrals = step_start.elapsed();
        timings.total = setup_start.elapsed();

        Ok(ScfIntegrals {
            overlap,
            kinetic,
            nuclear_attraction,
            electron_repulsion,
            timings,
        })
    }
}
