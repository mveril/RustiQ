// src/scf_calculation.rs

use crate::{
    eri::electron_repulsion_ints,
    math_utils::{assert_is_symmetric, is_positive_definite},
};
use nalgebra::{DMatrix, DVector};
use ndarray::Array4;
use rayon::prelude::*;

use crate::basis::gaussian::basis::Basis;
use crate::molecules::molecule::Molecule;

use super::{
    core::core_hamiltonian_ints,
    density_guess::DensityGuess,
    diis::{DiisAccelerator, DiisError},
};

/// Structure for an SCF calculation.
pub struct ScfCalculation<'a> {
    pub molecule: &'a Molecule,
    pub basis: &'a Basis,
    /// Maximum number of iterations for SCF convergence.
    pub max_iterations: usize,
    /// Convergence threshold for the energy difference between two successive iterations.
    pub convergence_threshold: f64,
    /// Current SCF energy.
    pub energy: f64,
    /// Molecular Orbital coefficients (matrix).
    pub mo_coefficients: DMatrix<f64>,
    /// Current electron density matrix.
    pub density_matrix: DMatrix<f64>,
    /// Fock matrix.
    pub fock_matrix: DMatrix<f64>,
    /// Current SCF residual norm from F(P) P S - S P F(P).
    pub residual_norm: f64,
    diis: Option<DiisAccelerator>,
    /// Two-electron integrals.
    pub two_electron_integrals: Array4<f64>,
    /// One-electron integrals - Hcore (kinetic + nuclear potential integrals combined).
    pub h_core: DMatrix<f64>,
    /// Kinetic energy matrix (T).
    pub t_matrix: DMatrix<f64>,
    /// Nuclear attraction energy matrix (V).
    pub v_matrix: DMatrix<f64>,
    /// Number of occupied orbitals (related to the number of electrons / 2 for closed-shell systems).
    pub occupied_orbitals: usize,
}

impl<'a> ScfCalculation<'a> {
    /// Creates a new `ScfCalculation` instance.
    pub fn new(
        molecule: &'a Molecule,
        basis: &'a Basis,
        max_iterations: usize,
        convergence_threshold: f64,
        density_guess_builder: Box<dyn DensityGuess>,
    ) -> Self {
        // Calculate the T and V matrices
        let (t_matrix, v_matrix) = core_hamiltonian_ints(molecule, basis);

        // H_core = T + V
        let h_core = &t_matrix + &v_matrix;

        // Calculate the two-electron integrals
        let two_electron_integrals: Array4<f64> = electron_repulsion_ints(basis);

        // Initialize density matrix using a density guess builder
        let density_matrix = density_guess_builder.build_density_guess(&h_core, molecule, basis);

        // Initial molecular orbital coefficients from diagonalization of H_core
        let (mo_coefficients, _) = Self::initial_mo_coefficients(&h_core, basis);

        // Initial Fock matrix
        let fock_matrix = h_core.clone(); // F = H_core initially

        Self {
            molecule,
            basis,
            max_iterations,
            convergence_threshold,
            energy: 0.0,
            mo_coefficients,
            density_matrix,
            fock_matrix,
            residual_norm: f64::INFINITY,
            diis: None,
            two_electron_integrals,
            h_core,
            t_matrix,
            v_matrix,
            occupied_orbitals: molecule.occupied_orbitals(),
        }
    }

    pub fn enable_diis(&mut self, diis_size: usize) -> Result<(), DiisError> {
        self.diis = Some(DiisAccelerator::try_new(diis_size)?);
        Ok(())
    }

    fn initial_mo_coefficients(
        h_core: &DMatrix<f64>,
        basis: &Basis,
    ) -> (DMatrix<f64>, DVector<f64>) {
        // Diagonalization of H_core to obtain initial MO coefficients
        let s = basis.overlap_ints(); // Overlap matrix S
        assert_is_symmetric(&s, 1e-8);
        assert!(
            is_positive_definite(&s),
            "Overlap matrix S is not positive definite."
        );
        let s_inv_sqrt = Self::symmetric_orthogonalizer(&s);
        let fock_preconditioned = &s_inv_sqrt.transpose() * h_core * &s_inv_sqrt;
        let eig = fock_preconditioned.symmetric_eigen();
        let mo_coefficients = &s_inv_sqrt * eig.eigenvectors;
        Self::sort_orbitals(mo_coefficients, eig.eigenvalues)
    }

    fn sort_orbitals(
        mo_coefficients: DMatrix<f64>,
        orbital_energies: DVector<f64>,
    ) -> (DMatrix<f64>, DVector<f64>) {
        let mut order: Vec<usize> = (0..orbital_energies.len()).collect();
        order.sort_by(|&a, &b| {
            orbital_energies[a]
                .partial_cmp(&orbital_energies[b])
                .expect("orbital energy should not be NaN")
        });

        let sorted_vectors = order
            .iter()
            .map(|&i| mo_coefficients.column(i).into_owned())
            .collect::<Vec<_>>();
        let sorted_energies =
            DVector::from_iterator(order.len(), order.iter().map(|&i| orbital_energies[i]));

        (DMatrix::from_columns(&sorted_vectors), sorted_energies)
    }

    fn symmetric_orthogonalizer(s: &DMatrix<f64>) -> DMatrix<f64> {
        let eig = s.clone().symmetric_eigen();
        let inv_sqrt_values = eig.eigenvalues.map(|value| {
            assert!(value > 0.0, "Overlap matrix S is not positive definite.");
            1.0 / value.sqrt()
        });
        let inv_sqrt_diag = DMatrix::from_diagonal(&inv_sqrt_values);
        &eig.eigenvectors * inv_sqrt_diag * eig.eigenvectors.transpose()
    }

    /// Execute the SCF calculation loop
    pub fn run(&mut self) {
        let mut energy_last = 0.0;

        for i in 0..self.max_iterations {
            // a. Update Fock matrix
            self.update_fock_matrix();
            self.apply_diis_if_enabled();

            // b. Solve Roothaan-Hall equation and update MO coefficients
            self.solve_roothaan_hall_equation();

            // c. Update density matrix
            self.update_density_matrix();

            // d. Calculate total energy
            self.update_total_energy();

            self.update_residual_norm();

            // e. Check for convergence
            let delta_energy = (self.energy - energy_last).abs();
            if delta_energy < self.convergence_threshold
                && self.residual_norm < self.convergence_threshold
            {
                println!("SCF converged after {} iterations.", i + 1);
                break;
            }

            if i == self.max_iterations - 1 {
                println!(
                    "SCF did not converge after {} iterations.",
                    self.max_iterations
                );
            }

            energy_last = self.energy;
        }

        // Calculate final energy including nuclear repulsion
        let nuclear_repulsion = self.molecule.geometry.nucl_repulsion();
        println!(
            "Total SCF Energy (without nuclear repulsion): {:.6} Hartree",
            self.energy
        );
        println!("Nuclear Repulsion Energy: {:.6} Hartree", nuclear_repulsion);
        println!(
            "Total Energy (including nuclear repulsion): {:.6} Hartree",
            self.energy + nuclear_repulsion
        );

        // Print detailed energy components
        self.print_energy_details();
    }

    fn update_fock_matrix(&mut self) {
        self.fock_matrix = self.build_fock_matrix(&self.density_matrix);
    }

    fn apply_diis_if_enabled(&mut self) {
        if let Some(diis) = &mut self.diis {
            let overlap_matrix = self.basis.overlap_ints();
            if let Some(fock_matrix) =
                diis.extrapolate(&self.fock_matrix, &self.density_matrix, &overlap_matrix)
            {
                self.fock_matrix = fock_matrix;
            }
        }
    }

    fn solve_roothaan_hall_equation(&mut self) {
        let (mo_coefficients, _orbital_energies) = self.solve_roothaan_hall();
        self.mo_coefficients = mo_coefficients;
    }

    fn update_density_matrix(&mut self) {
        self.density_matrix = self.calculate_density_matrix();
    }

    fn update_total_energy(&mut self) {
        self.energy = self.calculate_total_energy();
    }

    fn update_residual_norm(&mut self) {
        self.residual_norm = self.calculate_residual_norm();
    }

    fn build_fock_matrix(&self, density_matrix: &DMatrix<f64>) -> DMatrix<f64> {
        let nbasis = self.basis.nbasis();
        let values = (0..nbasis * nbasis)
            .into_par_iter()
            .map(|index| {
                let mu = index % nbasis;
                let nu = index / nbasis;
                let g_term: f64 = (0..nbasis)
                    .flat_map(|lambda| {
                        (0..nbasis).map(move |sigma| {
                            let density_element = density_matrix[(lambda, sigma)];
                            let two_electron_term =
                                self.two_electron_integrals[(mu, nu, lambda, sigma)];
                            let exchange_term =
                                self.two_electron_integrals[(mu, sigma, lambda, nu)];
                            density_element * (two_electron_term - 0.5 * exchange_term)
                        })
                    })
                    .sum();

                self.h_core[(mu, nu)] + g_term
            })
            .collect::<Vec<_>>();

        DMatrix::from_column_slice(nbasis, nbasis, &values)
    }

    fn solve_roothaan_hall(&self) -> (DMatrix<f64>, DVector<f64>) {
        let s = self.basis.overlap_ints(); // Overlap matrix S
        let s_inv_sqrt = Self::symmetric_orthogonalizer(&s);
        let fock_preconditioned = &s_inv_sqrt.transpose() * &self.fock_matrix * &s_inv_sqrt;
        let eig = fock_preconditioned.symmetric_eigen();
        let mo_coefficients = &s_inv_sqrt * eig.eigenvectors;
        Self::sort_orbitals(mo_coefficients, eig.eigenvalues)
    }

    fn calculate_density_matrix(&self) -> DMatrix<f64> {
        let nbasis = self.basis.nbasis();
        let values = (0..nbasis * nbasis)
            .into_par_iter()
            .map(|index| {
                let mu = index % nbasis;
                let nu = index / nbasis;
                let sum: f64 = (0..self.occupied_orbitals)
                    .map(|i| self.mo_coefficients[(mu, i)] * self.mo_coefficients[(nu, i)])
                    .sum();
                2.0 * sum
            })
            .collect::<Vec<_>>();

        DMatrix::from_column_slice(nbasis, nbasis, &values)
    }

    fn calculate_total_energy(&self) -> f64 {
        let ((kinetic, nuclear), electron_repulsion) = rayon::join(
            || {
                rayon::join(
                    || self.calculate_kinetic_energy(),
                    || self.calculate_nuclear_attraction_energy(),
                )
            },
            || self.calculate_electron_repulsion_energy(),
        );
        kinetic + nuclear + electron_repulsion
    }

    fn calculate_residual_norm(&self) -> f64 {
        let next_fock_matrix = self.build_fock_matrix(&self.density_matrix);
        Self::scf_residual_norm(
            &next_fock_matrix,
            &self.density_matrix,
            &self.basis.overlap_ints(),
        )
    }

    fn scf_residual_norm(
        fock_matrix: &DMatrix<f64>,
        density_matrix: &DMatrix<f64>,
        overlap_matrix: &DMatrix<f64>,
    ) -> f64 {
        DiisAccelerator::error_matrix(fock_matrix, density_matrix, overlap_matrix)
            .as_slice()
            .par_iter()
            .map(|value| value * value)
            .sum::<f64>()
            .sqrt()
    }

    fn calculate_kinetic_energy(&self) -> f64 {
        let nbasis = self.basis.nbasis();
        (0..nbasis * nbasis)
            .into_par_iter()
            .map(|index| {
                let mu = index % nbasis;
                let nu = index / nbasis;
                self.density_matrix[(mu, nu)] * self.t_matrix[(mu, nu)]
            })
            .sum()
    }

    fn calculate_nuclear_attraction_energy(&self) -> f64 {
        let nbasis = self.basis.nbasis();
        (0..nbasis * nbasis)
            .into_par_iter()
            .map(|index| {
                let mu = index % nbasis;
                let nu = index / nbasis;
                self.density_matrix[(mu, nu)] * self.v_matrix[(mu, nu)]
            })
            .sum()
    }

    fn calculate_electron_repulsion_energy(&self) -> f64 {
        let nbasis = self.basis.nbasis();
        let e_re: f64 = (0..nbasis * nbasis * nbasis * nbasis)
            .into_par_iter()
            .map(|index| {
                let mu = index % nbasis;
                let nu = (index / nbasis) % nbasis;
                let lambda = (index / (nbasis * nbasis)) % nbasis;
                let sigma = index / (nbasis * nbasis * nbasis);
                let eri_mu_nu_lambda_sigma = self.two_electron_integrals[(mu, nu, lambda, sigma)];
                let eri_mu_sigma_lambda_nu = self.two_electron_integrals[(mu, sigma, lambda, nu)];

                self.density_matrix[(mu, nu)]
                    * self.density_matrix[(lambda, sigma)]
                    * (eri_mu_nu_lambda_sigma - 0.5 * eri_mu_sigma_lambda_nu)
            })
            .sum();
        0.5 * e_re
    }

    fn print_energy_details(&self) {
        let kinetic = self.calculate_kinetic_energy();
        let nuclear = self.calculate_nuclear_attraction_energy();
        let electron_repulsion = self.calculate_electron_repulsion_energy();
        println!("Energy Details:");
        println!("  Kinetic Energy: {:.6} Hartree", kinetic);
        println!("  Nuclear Attraction Energy: {:.6} Hartree", nuclear);
        println!(
            "  Electron Repulsion Energy: {:.6} Hartree",
            electron_repulsion
        );
        println!(
            "  Total SCF Energy (without nuclear repulsion): {:.6} Hartree",
            self.energy
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basis::gaussian;
    use crate::molecules::atom::Atom;
    use crate::molecules::geometry::Geometry;
    use crate::molecules::units::Units;
    use crate::test_utils;
    use approx::assert_abs_diff_eq;
    use nalgebra::point;

    /// Simple implementation of DensityGuess for testing purposes.
    struct TestDensityGuess;

    impl DensityGuess for TestDensityGuess {
        fn build_density_guess(
            &self,
            _h_core: &DMatrix<f64>,
            _molecule: &Molecule,
            basis: &gaussian::basis::Basis,
        ) -> DMatrix<f64> {
            // Simple initial guess: identity matrix scaled by 1.0
            DMatrix::identity(basis.nbasis(), basis.nbasis())
        }
    }

    /// Helper function to create a Geometry for H2 molecule.
    fn create_h2_geometry() -> Geometry {
        let elements = periodic_table::periodic_table();
        let h = &elements[0]; // Hydrogen
        let atom1 = Atom::new(h, point![0.0, 0.0, -1.40]); // 0.74 Å ≈ 1.40 Bohr
        let atom2 = Atom::new(h, point![0.0, 0.0, 1.40]);
        Geometry::new(
            "Hydrogen molecule (H2)".to_string(),
            vec![atom1, atom2],
            Some(Units::Bohr),
            Some(Units::Bohr),
        )
    }

    #[test]
    fn test_initial_mo_coefficients() {
        let basis_file = test_utils::load_minimal_basis_file();
        let geometry = create_h2_geometry();
        let basis = Basis::load(&basis_file, &geometry);

        let h_core = &basis.kinetic_ints() + &basis.overlap_ints(); // Simplified H_core for testing

        let (mo_coeff, orbital_energies) = ScfCalculation::initial_mo_coefficients(&h_core, &basis);

        // Vérifier les dimensions
        assert_eq!(mo_coeff.nrows(), basis.nbasis());
        assert_eq!(mo_coeff.ncols(), basis.nbasis());
        assert_eq!(orbital_energies.len(), basis.nbasis());

        // Les orbitales moléculaires sont orthonormées dans la métrique AO: C^T S C = I.
        let overlap = basis.overlap_ints();
        let identity = &mo_coeff.transpose() * overlap * &mo_coeff;
        for i in 0..basis.nbasis() {
            for j in 0..basis.nbasis() {
                if i == j {
                    assert!(
                        (identity[(i, j)] - 1.0).abs() < 1e-6,
                        "Orthogonalité échouée pour i={}, j={}",
                        i,
                        j
                    );
                } else {
                    assert!(
                        identity[(i, j)].abs() < 1e-6,
                        "Orthogonalité échouée pour i={}, j={}",
                        i,
                        j
                    );
                }
            }
        }
    }

    #[test]
    fn test_build_fock_matrix() {
        let basis_file = test_utils::load_minimal_basis_file();
        let geometry = create_h2_geometry();
        let basis = Basis::load(&basis_file, &geometry);
        let molecule = Molecule::from(geometry);

        let (t_matrix, v_matrix) = core_hamiltonian_ints(&molecule, &basis);
        let _h_core = &t_matrix + &v_matrix;

        let _two_electron_integrals = electron_repulsion_ints(&basis);

        let density_guess = Box::new(TestDensityGuess);
        let scf = ScfCalculation::new(&molecule, &basis, 10, 1e-6, density_guess);

        let fock = scf.build_fock_matrix(&scf.density_matrix);

        // Avec une densité identité et des intégrales à deux électrons nulles, Fock devrait être H_core
        // Cependant, ici nous avons des intégrales réelles, donc ce test est plus complexe
        // Pour simplifier, nous pouvons vérifier la symétrie de la matrice de Fock
        for mu in 0..basis.nbasis() {
            for nu in 0..basis.nbasis() {
                assert!(
                    (fock[(mu, nu)] - fock[(nu, mu)]).abs() < 1e-6,
                    "Fock matrix is not symmetric at ({}, {})",
                    mu,
                    nu
                );
            }
        }
    }

    #[test]
    fn test_calculate_density_matrix() {
        let basis_file = test_utils::load_minimal_basis_file();
        let geometry = create_h2_geometry();
        let basis = Basis::load(&basis_file, &geometry);
        let molecule = Molecule::from(geometry);

        let density_guess = Box::new(TestDensityGuess);
        let scf = ScfCalculation::new(&molecule, &basis, 10, 1e-6, density_guess);

        let density = scf.calculate_density_matrix();

        for mu in 0..basis.nbasis() {
            for nu in 0..basis.nbasis() {
                assert!(
                    (density[(mu, nu)] - density[(nu, mu)]).abs() < 1e-10,
                    "Density matrix is not symmetric at ({}, {})",
                    mu,
                    nu
                );
            }
        }

        let electron_count = (density * basis.overlap_ints()).trace();
        assert!(
            (electron_count - molecule.total_electrons() as f64).abs() < 1e-8,
            "Density matrix electron count is {}, expected {}",
            electron_count,
            molecule.total_electrons()
        );
    }

    #[test]
    fn test_scf_convergence_h2() {
        let basis_file = test_utils::load_minimal_basis_file();
        let geometry = create_h2_geometry();
        let basis = Basis::load(&basis_file, &geometry);
        let molecule = Molecule::from(geometry);

        let density_guess = Box::new(TestDensityGuess);
        let mut scf = ScfCalculation::new(
            &molecule,
            &basis,
            50, // Augmenter le nombre maximum d'itérations si nécessaire
            1e-6,
            density_guess,
        );

        // Exécuter SCF
        scf.run();

        // Vérifier que l'énergie a été mise à jour
        // Pour ce test minimal, nous nous attendons à ce que l'énergie soit non nulle
        // En pratique, une comparaison avec une valeur théorique ou une référence est préférable
        assert!(
            scf.energy.abs() > 0.0,
            "L'énergie SCF devrait être non nulle après convergence."
        );
    }

    #[test]
    fn test_scf_h2_sto3g_matches_pyscf_reference_energy() {
        const PYSCF_ELECTRONIC_ENERGY: f64 = -1.831863646477507;
        const PYSCF_NUCLEAR_REPULSION_ENERGY: f64 = 0.715104339081081;
        const PYSCF_TOTAL_ENERGY: f64 = -1.116759307396425;

        let result = test_utils::run_sto3g_scf_for_sample("samples/h2/molecule.xyz");

        assert_abs_diff_eq!(
            result.electronic_energy,
            PYSCF_ELECTRONIC_ENERGY,
            epsilon = 1e-8
        );
        assert_abs_diff_eq!(
            result.nuclear_repulsion_energy,
            PYSCF_NUCLEAR_REPULSION_ENERGY,
            epsilon = 1e-8
        );
        assert_abs_diff_eq!(result.total_energy, PYSCF_TOTAL_ENERGY, epsilon = 1e-8);
    }

    #[test]
    fn test_scf_h2o_sto3g_matches_pyscf_reference_energy() {
        const PYSCF_ELECTRONIC_ENERGY: f64 = -84.151321547473785;
        const PYSCF_NUCLEAR_REPULSION_ENERGY: f64 = 9.188258417746113;
        const PYSCF_TOTAL_ENERGY: f64 = -74.963063129727672;

        let result = test_utils::run_sto3g_scf_for_sample("samples/h2o/h2o.xyz");

        assert_abs_diff_eq!(
            result.electronic_energy,
            PYSCF_ELECTRONIC_ENERGY,
            epsilon = 1e-8
        );
        assert_abs_diff_eq!(
            result.nuclear_repulsion_energy,
            PYSCF_NUCLEAR_REPULSION_ENERGY,
            epsilon = 1e-8
        );
        assert_abs_diff_eq!(result.total_energy, PYSCF_TOTAL_ENERGY, epsilon = 1e-8);
    }

    #[test]
    fn test_scf_residual_norm_is_small_after_convergence() {
        let geometry = test_utils::load_sample_geometry("samples/h2o/h2o.xyz");
        let basis = test_utils::load_sto3g_basis(&geometry);
        let molecule = Molecule::from(geometry);
        let mut scf = test_utils::new_one_electron_scf(&molecule, &basis, 100, 1e-8);

        scf.run();

        assert!(
            scf.residual_norm < 1e-8,
            "SCF residual norm is {}, expected < 1e-8",
            scf.residual_norm
        );
    }

    #[test]
    fn test_diis_scf_h2o_sto3g_matches_pyscf_reference_energy() {
        const PYSCF_ELECTRONIC_ENERGY: f64 = -84.151321547473785;

        let geometry = test_utils::load_sample_geometry("samples/h2o/h2o.xyz");
        let basis = test_utils::load_sto3g_basis(&geometry);
        let molecule = Molecule::from(geometry);
        let mut scf = test_utils::new_one_electron_scf(&molecule, &basis, 100, 1e-8);
        scf.enable_diis(6).unwrap();

        scf.run();

        assert_abs_diff_eq!(scf.energy, PYSCF_ELECTRONIC_ENERGY, epsilon = 1e-8);
        assert!(
            scf.residual_norm < 1e-8,
            "SCF residual norm is {}, expected < 1e-8",
            scf.residual_norm
        );
    }

    fn assert_symmetric_matrix(matrix: &DMatrix<f64>, epsilon: f64, label: &str) {
        for i in 0..matrix.nrows() {
            for j in 0..matrix.ncols() {
                assert!(
                    (matrix[(i, j)] - matrix[(j, i)]).abs() <= epsilon,
                    "{} is not symmetric at ({}, {})",
                    label,
                    i,
                    j
                );
            }
        }
    }

    #[test]
    fn test_scf_matrix_symmetry_invariants() {
        for path in ["samples/h2/molecule.xyz", "samples/h2o/h2o.xyz"] {
            let geometry = test_utils::load_sample_geometry(path);
            let basis = test_utils::load_sto3g_basis(&geometry);
            let molecule = Molecule::from(geometry);
            let mut scf = test_utils::new_one_electron_scf(&molecule, &basis, 100, 1e-8);

            assert_symmetric_matrix(&basis.overlap_ints(), 1e-10, "S");
            assert_symmetric_matrix(&scf.t_matrix, 1e-10, "T");
            assert_symmetric_matrix(&scf.v_matrix, 1e-10, "V");
            assert_symmetric_matrix(&scf.h_core, 1e-10, "Hcore");

            scf.run();

            assert_symmetric_matrix(&scf.fock_matrix, 1e-8, "F");
            assert_symmetric_matrix(&scf.density_matrix, 1e-8, "density");
        }
    }
}
