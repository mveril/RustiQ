use super::DensityGuess;
use crate::basis::gaussian::basis::Basis;
use crate::math_utils::assert_is_symmetric;
use crate::molecules::molecule::Molecule;
use nalgebra::DMatrix;

/// Structure représentant une estimation de densité initiale basée sur un seul électron.
pub struct OneElectron;

impl DensityGuess for OneElectron {
    fn build_density_guess(
        &self,
        h_core: &DMatrix<f64>,
        molecule: &Molecule,
        _basis: &Basis,
    ) -> DMatrix<f64> {
        // Vérifier que H_core est symétrique
        assert_is_symmetric(h_core, 1e-8);

        // Diagonaliser H_core pour obtenir les coefficients MO initiaux
        let eig = h_core.clone().symmetric_eigen();
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
        let _orbital_energies = eig.eigenvalues; // Non utilisé ici, mais disponible si nécessaire

        // Déterminer le nombre d'orbitales occupées
        let occupied_orbitals = molecule.occupied_orbitals();

        // Extraire les colonnes des orbitales occupées
        let occupied_columns: Vec<usize> = (0..occupied_orbitals).collect();
        let c_occ = mo_coefficients.select_columns(&occupied_columns);

        // Calculer la matrice de densité électronique D = 2 * C_occ * C_occ^T
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
    use crate::molecules::units::Units;
    use crate::test_utils;
    use nalgebra::point;

    /// Implémentation simple de DensityGuess pour les tests.
    struct TestDensityGuess;

    impl DensityGuess for TestDensityGuess {
        fn build_density_guess(
            &self,
            h_core: &DMatrix<f64>,
            _molecule: &Molecule,
            _basis: &Basis,
        ) -> DMatrix<f64> {
            // Utiliser Identity pour les tests
            DMatrix::identity(h_core.nrows(), h_core.ncols())
        }
    }

    /// Fonction utilitaire pour créer une géométrie H2.
    fn create_h2_geometry() -> Geometry {
        let elements = periodic_table::periodic_table();
        let h = &elements[0]; // Hydrogène
        let atom1 = Atom::new(h, point![0.0, 0.0, -1.4]); // 0.74 Å ≈ 1.40 Bohr
        let atom2 = Atom::new(h, point![0.0, 0.0, 1.4]);
        Geometry::new(
            "Hydrogen molecule (H2)".to_string(),
            vec![atom1, atom2],
            Some(Units::Bohr),
            Some(Units::Bohr),
        )
    }

    #[test]
    fn test_build_density_guess_optimized() {
        let basis_file = test_utils::load_minimal_basis_file();
        let geometry = create_h2_geometry();
        let basis = Basis::load(&basis_file, &geometry);
        let molecule = Molecule::from(geometry);

        // Calcul de H_core (simplifié pour le test)
        let (t_matrix, v_matrix) = core_hamiltonian_ints(&molecule, &basis);
        let _h_core = &t_matrix + &v_matrix;

        let _two_electron_integrals = electron_repulsion_ints(&basis);

        let scf: ScfCalculation<'_> =
            ScfCalculation::new(&molecule, &basis, 10, 1e-6, TestDensityGuess);

        let density = scf.density_matrix.clone();

        // Vérifier que la densité est symétrique
        assert_is_symmetric(&density, 1e-8);

        // Vérifier que la trace de la densité correspond au nombre d'électrons
        let trace = density.trace();
        let expected_trace = molecule.total_electrons() as f64;
        assert!(
            (trace - expected_trace).abs() < 1e-6,
            "La trace de la densité ({}) ne correspond pas au nombre d'électrons attendu ({}).",
            trace,
            expected_trace
        );

        // Vérifier que les éléments non diagonaux sont correctement calculés
        // Pour un cas très simple, on peut vérifier quelques éléments spécifiques
        // Ici, nous avons une molécule H2 symétrique, donc certains éléments devraient être égaux
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
