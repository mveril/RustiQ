use nalgebra::DMatrix;

use crate::{
    basis::gaussian::basis::Basis,
    config::validated::PositiveFiniteF64,
    eri::{electron_repulsion_ints_with_threshold, CompactEri, EriError},
    molecules::molecule::Molecule,
    persistence::EriCache,
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
    eri_schwarz_threshold: Option<PositiveFiniteF64>,
    eri_cache: Option<&'a EriCache>,
}

impl<'a> IntegralBuilder<'a> {
    pub(crate) fn new(
        molecule: &'a Molecule,
        basis: &'a Basis,
        eri_schwarz_threshold: Option<PositiveFiniteF64>,
        eri_cache: Option<&'a EriCache>,
    ) -> Self {
        Self {
            molecule,
            basis,
            eri_schwarz_threshold,
            eri_cache,
        }
    }

    pub(crate) fn core_hamiltonian(&self) -> (DMatrix<f64>, DMatrix<f64>) {
        core_hamiltonian_ints(self.molecule, self.basis)
    }

    pub(crate) fn overlap(&self) -> DMatrix<f64> {
        self.basis.overlap_ints()
    }

    pub(crate) fn electron_repulsion(&self) -> Result<CompactEri, EriError> {
        if let Some(eri) = self
            .eri_cache
            .and_then(|cache| cache.load(self.molecule, self.basis, self.eri_schwarz_threshold))
        {
            return Ok(eri);
        }
        let eri = electron_repulsion_ints_with_threshold(self.basis, self.eri_schwarz_threshold)?;
        if let Some(cache) = self.eri_cache {
            // Caching is an optimization: cache I/O never invalidates a calculation.
            let _ = cache.store(self.molecule, self.basis, self.eri_schwarz_threshold, &eri);
        }
        Ok(eri)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::DEFAULT_ERI_SCHWARZ_THRESHOLD,
        molecules::{molecule::Molecule, units::Units},
        test_utils::{load_sample_geometry_in_bohr, load_sto3g_basis},
    };

    #[test]
    fn cache_reuse_preserves_ao_eri_values() {
        let molecule = Molecule::try_new(
            load_sample_geometry_in_bohr("samples/h2/molecule.xyz"),
            Units::Bohr,
            0,
            std::num::NonZeroU8::MIN,
        )
        .unwrap();
        let basis = load_sto3g_basis(molecule.geometry());
        let threshold = Some(PositiveFiniteF64::try_new(DEFAULT_ERI_SCHWARZ_THRESHOLD).unwrap());
        let temporary = tempfile::tempdir().unwrap();
        let cache = EriCache::new(temporary.path());

        let uncached = IntegralBuilder::new(&molecule, &basis, threshold, None)
            .electron_repulsion()
            .unwrap();
        let cached = IntegralBuilder::new(&molecule, &basis, threshold, Some(&cache))
            .electron_repulsion()
            .unwrap();
        let reused = IntegralBuilder::new(&molecule, &basis, threshold, Some(&cache))
            .electron_repulsion()
            .unwrap();

        assert_eq!(cached.ordered_values(), uncached.ordered_values());
        assert_eq!(reused.ordered_values(), uncached.ordered_values());
        assert_eq!(cache.entries().unwrap().len(), 1);
    }
}
