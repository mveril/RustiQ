use nalgebra::DMatrix;

use crate::{
    basis::gaussian::basis::Basis,
    config::validated::NonNegativeFiniteF64,
    eri::{electron_repulsion_ints_with_threshold, CompactEri, EriError},
    molecules::molecule::Molecule,
};

use super::core::core_hamiltonian_ints;

/// Integral data shared by restricted and unrestricted SCF calculations.
pub(crate) struct ScfIntegrals {
    pub(crate) overlap: DMatrix<f64>,
    pub(crate) kinetic: DMatrix<f64>,
    pub(crate) nuclear_attraction: DMatrix<f64>,
    pub(crate) electron_repulsion: CompactEri,
}

/// Produces the one- and two-electron integrals used by SCF.
pub(crate) struct IntegralBuilder<'a> {
    molecule: &'a Molecule,
    basis: &'a Basis,
    eri_schwarz_threshold: Option<NonNegativeFiniteF64>,
}

impl<'a> IntegralBuilder<'a> {
    pub(crate) fn new(
        molecule: &'a Molecule,
        basis: &'a Basis,
        eri_schwarz_threshold: Option<NonNegativeFiniteF64>,
    ) -> Self {
        Self {
            molecule,
            basis,
            eri_schwarz_threshold,
        }
    }

    pub(crate) fn core_hamiltonian(&self) -> (DMatrix<f64>, DMatrix<f64>) {
        core_hamiltonian_ints(self.molecule, self.basis)
    }

    pub(crate) fn overlap(&self) -> DMatrix<f64> {
        self.basis.overlap_ints()
    }

    pub(crate) fn electron_repulsion(&self) -> Result<CompactEri, EriError> {
        electron_repulsion_ints_with_threshold(self.basis, self.eri_schwarz_threshold)
    }
}
