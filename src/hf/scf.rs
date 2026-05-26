// src/scf_calculation.rs

use std::time::Instant;

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
    scf_energy_details::ScfEnergyDetails,
    scf_iteration::ScfIteration,
    scf_observer::{NoopScfObserver, ScfObserver},
    scf_result::{ScfResult, ScfSetupTimings, ScfTimings},
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
    /// Overlap matrix (S).
    overlap_matrix: DMatrix<f64>,
    /// Symmetric orthogonalizer (S^(-1/2)).
    s_inv_sqrt: DMatrix<f64>,
    /// Number of occupied orbitals (related to the number of electrons / 2 for closed-shell systems).
    pub occupied_orbitals: usize,
    timings: ScfTimings,
}

impl<'a> ScfCalculation<'a> {
    /// Creates a new `ScfCalculation` instance.
    #[allow(dead_code)]
    pub fn new<G>(
        molecule: &'a Molecule,
        basis: &'a Basis,
        max_iterations: usize,
        convergence_threshold: f64,
        density_guess_builder: G,
    ) -> Result<Self, G::Error>
    where
        G: DensityGuess,
    {
        Self::new_with_progress(
            molecule,
            basis,
            max_iterations,
            convergence_threshold,
            density_guess_builder,
            |_| {},
        )
    }

    pub fn new_with_progress<G, F>(
        molecule: &'a Molecule,
        basis: &'a Basis,
        max_iterations: usize,
        convergence_threshold: f64,
        density_guess_builder: G,
        mut progress: F,
    ) -> Result<Self, G::Error>
    where
        G: DensityGuess,
        F: FnMut(&str),
    {
        let setup_start = Instant::now();
        let mut setup_timings = ScfSetupTimings::default();

        // Calculate the T and V matrices
        progress("Building one-electron core Hamiltonian");
        let step_start = Instant::now();
        let (t_matrix, v_matrix) = core_hamiltonian_ints(molecule, basis);
        setup_timings.core_hamiltonian = step_start.elapsed();

        // H_core = T + V
        let h_core = &t_matrix + &v_matrix;

        progress("Building overlap matrix");
        let step_start = Instant::now();
        let overlap_matrix = basis.overlap_ints();
        setup_timings.overlap = step_start.elapsed();
        assert_is_symmetric(&overlap_matrix, 1e-8);
        assert!(
            is_positive_definite(&overlap_matrix),
            "Overlap matrix S is not positive definite."
        );

        progress("Building symmetric orthogonalizer");
        let step_start = Instant::now();
        let s_inv_sqrt = Self::symmetric_orthogonalizer(&overlap_matrix);
        setup_timings.orthogonalizer = step_start.elapsed();

        // Calculate the two-electron integrals
        progress("Building electron repulsion integrals");
        let step_start = Instant::now();
        let two_electron_integrals: Array4<f64> = electron_repulsion_ints(basis);
        setup_timings.electron_repulsion_integrals = step_start.elapsed();

        // Initialize density matrix using a density guess builder
        progress("Building initial density guess");
        let step_start = Instant::now();
        let density_matrix = density_guess_builder.build_density_guess(&h_core, molecule, basis)?;
        setup_timings.density_guess = step_start.elapsed();

        // Initial molecular orbital coefficients from diagonalization of H_core
        progress("Building initial molecular orbitals");
        let step_start = Instant::now();
        let (mo_coefficients, _) = Self::initial_mo_coefficients(&h_core, &s_inv_sqrt);
        setup_timings.initial_orbitals = step_start.elapsed();

        // Initial Fock matrix
        let fock_matrix = h_core.clone(); // F = H_core initially
        setup_timings.total = setup_start.elapsed();

        Ok(Self {
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
            overlap_matrix,
            s_inv_sqrt,
            occupied_orbitals: molecule.occupied_orbitals(),
            timings: ScfTimings {
                setup: setup_timings,
                ..ScfTimings::default()
            },
        })
    }

    pub fn enable_diis(&mut self, diis_size: usize) -> Result<(), DiisError> {
        self.diis = Some(DiisAccelerator::try_new(diis_size)?);
        Ok(())
    }

    fn initial_mo_coefficients(
        h_core: &DMatrix<f64>,
        s_inv_sqrt: &DMatrix<f64>,
    ) -> (DMatrix<f64>, DVector<f64>) {
        // Diagonalization of H_core to obtain initial MO coefficients
        let fock_preconditioned = &s_inv_sqrt.transpose() * h_core * s_inv_sqrt;
        let eig = fock_preconditioned.symmetric_eigen();
        let mo_coefficients = s_inv_sqrt * eig.eigenvectors;
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
    #[allow(dead_code)]
    pub fn run(&mut self) -> ScfResult {
        let mut observer = NoopScfObserver;
        self.run_with_observer(&mut observer)
    }

    pub fn run_with_observer<O>(&mut self, observer: &mut O) -> ScfResult
    where
        O: ScfObserver,
    {
        let mut energy_last = 0.0;
        let mut converged = false;
        let mut iterations = 0;
        let mut delta_energy = f64::INFINITY;
        let run_start = Instant::now();
        let iterations_start = Instant::now();

        for i in 0..self.max_iterations {
            iterations = i + 1;

            // a. Update Fock matrix
            if i == 0 {
                self.update_fock_matrix();
            }
            self.apply_diis_if_enabled();

            // b. Solve Roothaan-Hall equation and update MO coefficients
            self.solve_roothaan_hall_equation();

            // c. Update density matrix
            self.update_density_matrix();

            // d. Build F(P) for the new density and calculate total energy
            self.update_residual_norm_and_next_fock();

            self.update_total_energy_from_current_fock();

            // e. Check for convergence
            delta_energy = (self.energy - energy_last).abs();
            let iteration = ScfIteration {
                iteration: iterations,
                electronic_energy: self.energy,
                delta_energy,
                residual_norm: self.residual_norm,
            };
            observer.on_iteration(&iteration);

            if delta_energy < self.convergence_threshold
                && self.residual_norm < self.convergence_threshold
            {
                converged = true;
                break;
            }

            energy_last = self.energy;
        }
        self.timings.iterations = iterations_start.elapsed();

        // Calculate final energy including nuclear repulsion
        let nuclear_repulsion = self.molecule.geometry.nucl_repulsion();
        let total_energy = self.energy + nuclear_repulsion;
        let final_energy_details_start = Instant::now();
        let energy_details = self.calculate_energy_details();
        self.timings.final_energy_details = final_energy_details_start.elapsed();
        self.timings.total = run_start.elapsed() + self.timings.setup.total;

        ScfResult {
            converged,
            iterations,
            electronic_energy: self.energy,
            nuclear_repulsion_energy: nuclear_repulsion,
            total_energy,
            delta_energy,
            residual_norm: self.residual_norm,
            energy_details,
            timings: self.timings.clone(),
        }
    }

    fn update_fock_matrix(&mut self) {
        self.fock_matrix = self.build_fock_matrix(&self.density_matrix);
    }

    fn apply_diis_if_enabled(&mut self) {
        if let Some(diis) = &mut self.diis {
            if let Some(fock_matrix) = diis.extrapolate(
                &self.fock_matrix,
                &self.density_matrix,
                &self.overlap_matrix,
            ) {
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

    fn update_total_energy_from_current_fock(&mut self) {
        self.energy = self.calculate_total_energy_from_current_fock();
    }

    fn update_residual_norm_and_next_fock(&mut self) {
        let next_fock_matrix = self.build_fock_matrix(&self.density_matrix);
        self.residual_norm = Self::scf_residual_norm(
            &next_fock_matrix,
            &self.density_matrix,
            &self.overlap_matrix,
        );
        self.fock_matrix = next_fock_matrix;
    }

    fn build_fock_matrix(&self, density_matrix: &DMatrix<f64>) -> DMatrix<f64> {
        let nbasis = self.basis.nbasis();
        let n_pairs = nbasis * (nbasis + 1) / 2;
        let values = (0..n_pairs)
            .into_par_iter()
            .map(|index| {
                let (mu, nu) = basis_function_pair(index);
                let mut g_term = 0.0;

                for lambda in 0..nbasis {
                    for sigma in 0..=lambda {
                        let density_element = density_matrix[(lambda, sigma)];
                        let coulomb_term = self.two_electron_integrals[(mu, nu, lambda, sigma)];
                        let exchange_term = self.two_electron_integrals[(mu, sigma, lambda, nu)];

                        if lambda == sigma {
                            g_term += density_element * (coulomb_term - 0.5 * exchange_term);
                        } else {
                            let swapped_exchange_term =
                                self.two_electron_integrals[(mu, lambda, sigma, nu)];
                            g_term += density_element
                                * (2.0 * coulomb_term
                                    - 0.5 * (exchange_term + swapped_exchange_term));
                        }
                    }
                }

                (mu, nu, self.h_core[(mu, nu)] + g_term)
            })
            .collect::<Vec<_>>();

        let mut fock_matrix = DMatrix::zeros(nbasis, nbasis);
        for (mu, nu, value) in values {
            fock_matrix[(mu, nu)] = value;
            if mu != nu {
                fock_matrix[(nu, mu)] = value;
            }
        }
        fock_matrix
    }

    fn solve_roothaan_hall(&self) -> (DMatrix<f64>, DVector<f64>) {
        let fock_preconditioned =
            &self.s_inv_sqrt.transpose() * &self.fock_matrix * &self.s_inv_sqrt;
        let eig = fock_preconditioned.symmetric_eigen();
        let mo_coefficients = &self.s_inv_sqrt * eig.eigenvectors;
        Self::sort_orbitals(mo_coefficients, eig.eigenvalues)
    }

    fn calculate_density_matrix(&self) -> DMatrix<f64> {
        let c_occ = self.mo_coefficients.columns(0, self.occupied_orbitals);
        2.0 * c_occ * c_occ.transpose()
    }

    fn calculate_total_energy_from_current_fock(&self) -> f64 {
        0.5 * self.density_matrix.dot(&(&self.h_core + &self.fock_matrix))
    }

    fn calculate_energy_details(&self) -> ScfEnergyDetails {
        let ((kinetic_energy, nuclear_attraction_energy), electron_repulsion_energy) = rayon::join(
            || {
                rayon::join(
                    || self.calculate_kinetic_energy(),
                    || self.calculate_nuclear_attraction_energy(),
                )
            },
            || self.calculate_electron_repulsion_energy(),
        );
        ScfEnergyDetails {
            kinetic_energy,
            nuclear_attraction_energy,
            electron_repulsion_energy,
        }
    }

    fn scf_residual_norm(
        fock_matrix: &DMatrix<f64>,
        density_matrix: &DMatrix<f64>,
        overlap_matrix: &DMatrix<f64>,
    ) -> f64 {
        DiisAccelerator::error_matrix(fock_matrix, density_matrix, overlap_matrix)
            .as_slice()
            .par_iter()
            .map(|value| value.powi(2))
            .sum::<f64>()
            .sqrt()
    }

    fn calculate_kinetic_energy(&self) -> f64 {
        self.density_matrix.dot(&self.t_matrix)
    }

    fn calculate_nuclear_attraction_energy(&self) -> f64 {
        self.density_matrix.dot(&self.v_matrix)
    }

    fn calculate_electron_repulsion_energy(&self) -> f64 {
        let nbasis = self.basis.nbasis();
        let e_re: f64 = (0..nbasis.pow(4))
            .into_par_iter()
            .map(|index| {
                let mu = index % nbasis;
                let nu = (index / nbasis) % nbasis;
                let lambda = (index / (nbasis.pow(2))) % nbasis;
                let sigma = index / (nbasis.pow(3));
                let eri_mu_nu_lambda_sigma = self.two_electron_integrals[(mu, nu, lambda, sigma)];
                let eri_mu_sigma_lambda_nu = self.two_electron_integrals[(mu, sigma, lambda, nu)];

                self.density_matrix[(mu, nu)]
                    * self.density_matrix[(lambda, sigma)]
                    * (eri_mu_nu_lambda_sigma - 0.5 * eri_mu_sigma_lambda_nu)
            })
            .sum();
        0.5 * e_re
    }
}

fn basis_function_pair(pair_index: usize) -> (usize, usize) {
    let first = (((8 * pair_index + 1) as f64).sqrt() as usize - 1) / 2;
    let second = pair_index - first * (first + 1) / 2;
    (first, second)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basis::gaussian;
    use crate::molecules::atom::Atom;
    use crate::molecules::geometry::Geometry;
    use crate::test_utils;
    use approx::assert_abs_diff_eq;
    use nalgebra::point;
    use std::convert::Infallible;

    /// Simple implementation of DensityGuess for testing purposes.
    struct TestDensityGuess;

    impl DensityGuess for TestDensityGuess {
        type Error = Infallible;
        fn build_density_guess(
            &self,
            _h_core: &DMatrix<f64>,
            _molecule: &Molecule,
            basis: &gaussian::basis::Basis,
        ) -> Result<DMatrix<f64>, Self::Error> {
            // Simple initial guess: identity matrix scaled by 1.0
            Ok(DMatrix::identity(basis.nbasis(), basis.nbasis()))
        }
    }

    /// Helper function to create a Geometry for H2 molecule.
    fn create_h2_geometry() -> Geometry {
        let elements = periodic_table::periodic_table();
        let h = &elements[0]; // Hydrogen
        let atom1 = Atom::new(h, point![0.0, 0.0, -1.40]); // 0.74 Å ≈ 1.40 Bohr
        let atom2 = Atom::new(h, point![0.0, 0.0, 1.40]);
        Geometry::new("Hydrogen molecule (H2)".to_string(), vec![atom1, atom2])
    }

    #[test]
    fn test_initial_mo_coefficients() {
        let basis_file = test_utils::load_minimal_basis_file();
        let geometry = create_h2_geometry();
        let basis = Basis::load(&basis_file, &geometry);

        let h_core = &basis.kinetic_ints() + &basis.overlap_ints(); // Simplified H_core for testing

        let overlap = basis.overlap_ints();
        let s_inv_sqrt = ScfCalculation::symmetric_orthogonalizer(&overlap);
        let (mo_coeff, orbital_energies) =
            ScfCalculation::initial_mo_coefficients(&h_core, &s_inv_sqrt);

        // Check dimensions
        assert_eq!(mo_coeff.nrows(), basis.nbasis());
        assert_eq!(mo_coeff.ncols(), basis.nbasis());
        assert_eq!(orbital_energies.len(), basis.nbasis());

        // Molecular orbitals are orthonormal in the AO metric: C^T S C = I.
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

        let scf = ScfCalculation::new(&molecule, &basis, 10, 1e-6, TestDensityGuess).unwrap();

        let fock = scf.build_fock_matrix(&scf.density_matrix);

        // With identity density and zero two-electron integrals, Fock should be H_core
        // However, this uses real integrals, so this test is more complex
        // To simplify, we can check the symmetry of the Fock matrix
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

        let scf = ScfCalculation::new(&molecule, &basis, 10, 1e-6, TestDensityGuess).unwrap();

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

        let mut scf = ScfCalculation::new(
            &molecule,
            &basis,
            50, // Increase the maximum number of iterations if needed
            1e-6,
            TestDensityGuess,
        )
        .unwrap();

        // Run SCF
        let result = scf.run();

        // Check that the energy has been updated
        // For this minimal test, we expect the energy to be non-zero
        // In practice, comparison with a theoretical or reference value is preferable
        assert!(result.converged);
        assert!(result.iterations <= 50);
        assert!(
            result.electronic_energy.abs() > 0.0,
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
        const PYSCF_ELECTRONIC_ENERGY: f64 = -84.151_321_547_473_78;
        const PYSCF_NUCLEAR_REPULSION_ENERGY: f64 = 9.188258417746113;
        const PYSCF_TOTAL_ENERGY: f64 = -74.963_063_129_727_67;

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
        let geometry = test_utils::load_sample_geometry_in_bohr("samples/h2o/h2o.xyz");
        let basis = test_utils::load_sto3g_basis(&geometry);
        let molecule = Molecule::from(geometry);
        let mut scf = test_utils::new_one_electron_scf(&molecule, &basis, 100, 1e-8);

        let result = scf.run();

        assert!(
            result.residual_norm < 1e-8,
            "SCF residual norm is {}, expected < 1e-8",
            result.residual_norm
        );
        assert!(result.converged);
        assert!(result.delta_energy < 1e-8);
    }

    #[test]
    fn test_diis_scf_h2o_sto3g_matches_pyscf_reference_energy() {
        const PYSCF_ELECTRONIC_ENERGY: f64 = -84.151_321_547_473_78;

        let geometry = test_utils::load_sample_geometry_in_bohr("samples/h2o/h2o.xyz");
        let basis = test_utils::load_sto3g_basis(&geometry);
        let molecule = Molecule::from(geometry);
        let mut scf = test_utils::new_one_electron_scf(&molecule, &basis, 100, 1e-8);
        scf.enable_diis(6).unwrap();

        let result = scf.run();

        assert!(result.converged);
        assert_abs_diff_eq!(
            result.electronic_energy,
            PYSCF_ELECTRONIC_ENERGY,
            epsilon = 1e-8
        );
        assert!(
            result.residual_norm < 1e-8,
            "SCF residual norm is {}, expected < 1e-8",
            result.residual_norm
        );
    }

    #[test]
    fn test_scf_result_reports_non_convergence() {
        let geometry = test_utils::load_sample_geometry_in_bohr("samples/h2o/h2o.xyz");
        let basis = test_utils::load_sto3g_basis(&geometry);
        let molecule = Molecule::from(geometry);
        let mut scf = test_utils::new_one_electron_scf(&molecule, &basis, 1, 1e-12);

        let result = scf.run();

        assert!(!result.converged);
        assert_eq!(result.iterations, 1);
        assert!(result.delta_energy.is_finite());
        assert!(result.residual_norm.is_finite());
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
            let geometry = test_utils::load_sample_geometry_in_bohr(path);
            let basis = test_utils::load_sto3g_basis(&geometry);
            let molecule = Molecule::from(geometry);
            let mut scf = test_utils::new_one_electron_scf(&molecule, &basis, 100, 1e-8);

            assert_symmetric_matrix(&basis.overlap_ints(), 1e-10, "S");
            assert_symmetric_matrix(&scf.t_matrix, 1e-10, "T");
            assert_symmetric_matrix(&scf.v_matrix, 1e-10, "V");
            assert_symmetric_matrix(&scf.h_core, 1e-10, "Hcore");

            let result = scf.run();

            assert!(result.converged);
            assert_symmetric_matrix(&scf.fock_matrix, 1e-8, "F");
            assert_symmetric_matrix(&scf.density_matrix, 1e-8, "density");
        }
    }
}
