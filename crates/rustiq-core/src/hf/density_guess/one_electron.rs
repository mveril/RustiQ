use super::{
    unrestricted_perturb_fock_like_matrices, DensityGuess, DensityGuessError, OrbitalGuess,
};
use crate::basis::gaussian::basis::Basis;
use crate::runfile::hf::GuessPerturbationConfig;
use nalgebra::DMatrix;

/// Structure representing an initial density estimate based on one electron.
#[derive(Default)]
pub struct OneElectron {
    perturbation: Option<GuessPerturbationConfig>,
}

impl OneElectron {
    pub(crate) fn new(perturbation: Option<GuessPerturbationConfig>) -> Self {
        Self { perturbation }
    }
}

impl DensityGuess for OneElectron {
    type Error = DensityGuessError;
    fn build_orbital_guess(
        &self,
        h_core: &DMatrix<f64>,
        _basis: &Basis,
    ) -> Result<OrbitalGuess, Self::Error> {
        crate::debug_assert_is_symmetric!(h_core, 1e-8);
        match self.perturbation {
            Some(perturbation) => {
                unrestricted_perturb_fock_like_matrices(h_core, perturbation).map_err(Into::into)
            }
            None => Ok(OrbitalGuess::CommonFockLike(h_core.clone())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eri::electron_repulsion_ints;
    use crate::hf::core::core_hamiltonian_ints;
    use crate::hf::density_guess::DensityGuess;
    use crate::hf::scf::ScfCalculation;
    use crate::molecules::atom::Atom;
    use crate::molecules::geometry::Geometry;
    use crate::molecules::molecule::Molecule;
    use crate::test_utils;
    use nalgebra::point;
    use std::convert::Infallible;

    /// Simple implementation of DensityGuess for tests.
    struct TestDensityGuess;

    impl DensityGuess for TestDensityGuess {
        type Error = Infallible;
        fn build_orbital_guess(
            &self,
            h_core: &DMatrix<f64>,
            _basis: &Basis,
        ) -> Result<OrbitalGuess, Self::Error> {
            Ok(OrbitalGuess::CommonFockLike(DMatrix::identity(
                h_core.nrows(),
                h_core.ncols(),
            )))
        }
    }

    /// Helper function to create an H2 geometry.
    fn create_h2_geometry() -> Geometry {
        let elements = periodic_table::periodic_table();
        let h = &elements[0]; // Hydrogen
        let atom1 = Atom::new(h, point![0.0, 0.0, -1.4]); // 0.74 Å ≈ 1.40 Bohr
        let atom2 = Atom::new(h, point![0.0, 0.0, 1.4]);
        Geometry::new("Hydrogen molecule (H2)".to_string(), vec![atom1, atom2])
    }

    #[test]
    fn test_build_density_guess_optimized() {
        let basis_file = test_utils::load_minimal_basis_file();
        let geometry = create_h2_geometry();
        let basis = Basis::try_load(&basis_file, &geometry).unwrap();
        let molecule = Molecule::try_new(
            geometry,
            crate::molecules::units::Units::Bohr,
            0,
            std::num::NonZeroU8::MIN,
        )
        .unwrap();

        // Calculate H_core (simplified for the test)
        let (t_matrix, v_matrix) = core_hamiltonian_ints(&molecule, &basis);
        let _h_core = &t_matrix + &v_matrix;

        let _two_electron_integrals = electron_repulsion_ints(&basis);

        let scf: ScfCalculation<'_> =
            ScfCalculation::new(&molecule, &basis, 10, 1e-6, 1e-8, TestDensityGuess).unwrap();

        let density = scf.density_matrix.clone();

        // Check that the density is symmetric
        crate::debug_assert_is_symmetric!(&density, 1e-8);

        // AO densities are normalized in the overlap metric, not the Euclidean trace.
        let trace = (&density * basis.overlap_ints()).trace();
        let expected_trace = molecule.total_electrons() as f64;
        assert!(
            (trace - expected_trace).abs() < 1e-6,
            "La trace de la densité ({}) ne correspond pas au nombre d'électrons attendu ({}).",
            trace,
            expected_trace
        );

        // Check that the off-diagonal elements are calculated correctly
        // For a very simple case, we can check a few specific elements
        // Here, we have a symmetric H2 molecule, so some elements should be equal
        for mu in 0..basis.nbasis() {
            for nu in 0..basis.nbasis() {
                assert!(
                    (density[(mu, nu)] - density[(nu, mu)]).abs() < 1e-8,
                    "La densité n'est pas symétrique en ({}, {}).",
                    mu,
                    nu
                );
            }
        }
    }
}
