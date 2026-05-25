use super::DensityGuess;
use crate::basis::gaussian::basis::Basis;
use crate::hf::density_guess::perturb_fock_like_matrix;
use crate::math_utils::assert_is_symmetric;
use crate::molecules::molecule::Molecule;
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
    fn build_density_guess(
        &self,
        h_core: &DMatrix<f64>,
        molecule: &Molecule,
        _basis: &Basis,
    ) -> DMatrix<f64> {
        // Check that H_core is symmetric
        assert_is_symmetric(h_core, 1e-8);
        let h_core = perturb_fock_like_matrix(h_core, self.perturbation);

        // Diagonalize H_core to obtain the initial MO coefficients
        let eig = h_core.symmetric_eigen();
        let mut order: Vec<usize> = (0..eig.eigenvalues.len()).collect();
        order.sort_by(|&a, &b| {
            eig.eigenvalues[a]
                .partial_cmp(&eig.eigenvalues[b])
                .expect("orbital energy should not be NaN")
        });
        let sorted_vectors = order
            .iter()
            .map(|&i| eig.eigenvectors.column(i).into_owned())
            .collect::<Vec<_>>();
        let mo_coefficients = DMatrix::from_columns(&sorted_vectors);
        let _orbital_energies = eig.eigenvalues; // Not used here, but available if needed

        // Determine the number of occupied orbitals
        let occupied_orbitals = molecule.occupied_orbitals();

        // Extract the occupied orbital columns
        let occupied_columns: Vec<usize> = (0..occupied_orbitals).collect();
        let c_occ = mo_coefficients.select_columns(&occupied_columns);

        // Calculate the electron density matrix D = 2 * C_occ * C_occ^T
        2.0 * &c_occ * &c_occ.transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eri::electron_repulsion_ints;
    use crate::hf::core::core_hamiltonian_ints;
    use crate::hf::density_guess::DensityGuess;
    use crate::hf::scf::ScfCalculation;
    use crate::math_utils::assert_is_symmetric;
    use crate::molecules::atom::Atom;
    use crate::molecules::geometry::Geometry;
    use crate::test_utils;
    use nalgebra::point;

    /// Simple implementation of DensityGuess for tests.
    struct TestDensityGuess;

    impl DensityGuess for TestDensityGuess {
        fn build_density_guess(
            &self,
            h_core: &DMatrix<f64>,
            _molecule: &Molecule,
            _basis: &Basis,
        ) -> DMatrix<f64> {
            // Use Identity for tests
            DMatrix::identity(h_core.nrows(), h_core.ncols())
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
        let basis = Basis::load(&basis_file, &geometry);
        let molecule = Molecule::from(geometry);

        // Calculate H_core (simplified for the test)
        let (t_matrix, v_matrix) = core_hamiltonian_ints(&molecule, &basis);
        let _h_core = &t_matrix + &v_matrix;

        let _two_electron_integrals = electron_repulsion_ints(&basis);

        let scf: ScfCalculation<'_> =
            ScfCalculation::new(&molecule, &basis, 10, 1e-6, TestDensityGuess);

        let density = scf.density_matrix.clone();

        // Check that the density is symmetric
        assert_is_symmetric(&density, 1e-8);

        // Check that the density trace matches the number of electrons
        let trace = density.trace();
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
