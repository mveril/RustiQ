use std::time::Instant;

use nalgebra::{DMatrix, DVector};
use rayon::prelude::*;
use thiserror::Error;

use crate::{
    basis::gaussian::basis::Basis,
    eri::{electron_repulsion_ints, CompactEri},
    hf::numerical_error::{
        ensure_finite_value, ensure_finite_values, ensure_positive_definite, NumericalError,
    },
    molecules::molecule::Molecule,
};

use super::{
    core::core_hamiltonian_ints,
    density_guess::DensityGuess,
    diis::{DiisAccelerator, DiisError},
    scf::ScfSetupError,
    scf_energy_details::ScfEnergyDetails,
    scf_iteration::ScfIteration,
    scf_observer::{NoopScfObserver, ScfObserver},
    scf_result::{ScfResult, ScfSetupTimings, ScfTimings},
};

#[derive(Debug, Error)]
pub(crate) enum UhfSetupError<E>
where
    E: std::error::Error + 'static,
{
    #[error(transparent)]
    Scf(#[from] ScfSetupError<E>),
    #[error("invalid open-shell electron configuration: total electrons = {electrons}, multiplicity = {multiplicity}")]
    InvalidElectronConfiguration { electrons: usize, multiplicity: u8 },
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Spin<T> {
    pub alpha: T,
    pub beta: T,
}

impl<T> Spin<T> {
    fn new(alpha: T, beta: T) -> Self {
        Self { alpha, beta }
    }

    fn zip_map<U, V>(self, other: Spin<U>, mut f: impl FnMut(T, U) -> V) -> Spin<V> {
        Spin::new(f(self.alpha, other.alpha), f(self.beta, other.beta))
    }
}

impl<T: Clone> Spin<T> {
    fn duplicate(value: T) -> Self {
        Self {
            alpha: value.clone(),
            beta: value,
        }
    }
}

pub(crate) type SpinDiisAccelerators = Spin<DiisAccelerator>;
pub(crate) type SpinMatrices = Spin<DMatrix<f64>>;
pub(crate) struct UhfCalculation<'a> {
    pub molecule: &'a Molecule,
    pub basis: &'a Basis,
    pub max_iterations: usize,
    pub convergence_threshold: f64,
    pub energy: f64,
    pub mo_coefficients: SpinMatrices,
    pub density: SpinMatrices,
    pub fock: SpinMatrices,
    pub residual_norm: f64,
    diis: Option<SpinDiisAccelerators>,
    pub two_electron_integrals: CompactEri,
    pub h_core: DMatrix<f64>,
    pub t_matrix: DMatrix<f64>,
    pub v_matrix: DMatrix<f64>,
    overlap_matrix: DMatrix<f64>,
    s_inv_sqrt: DMatrix<f64>,
    pub occupied_orbitals: Spin<usize>,
    timings: ScfTimings,
}

impl<'a> UhfCalculation<'a> {
    #[allow(dead_code)]
    pub fn new<G>(
        molecule: &'a Molecule,
        basis: &'a Basis,
        max_iterations: usize,
        convergence_threshold: f64,
        density_guess_builder: G,
    ) -> Result<Self, UhfSetupError<G::Error>>
    where
        G: DensityGuess,
        G::Error: 'static,
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
    ) -> Result<Self, UhfSetupError<G::Error>>
    where
        G: DensityGuess,
        G::Error: 'static,
        F: FnMut(&str),
    {
        let occupied_orbitals = alpha_beta_occupied_orbitals(molecule)?;
        let setup_start = Instant::now();
        let mut setup_timings = ScfSetupTimings::default();

        progress("Building one-electron core Hamiltonian");
        let step_start = Instant::now();
        let (t_matrix, v_matrix) = core_hamiltonian_ints(molecule, basis);
        setup_timings.core_hamiltonian = step_start.elapsed();
        let h_core = &t_matrix + &v_matrix;

        progress("Building overlap matrix");
        let step_start = Instant::now();
        let overlap_matrix = basis.overlap_ints();
        setup_timings.overlap = step_start.elapsed();
        crate::debug_assert_is_symmetric!(&overlap_matrix, 1e-8);
        ensure_positive_definite(&overlap_matrix, "overlap").map_err(ScfSetupError::Numerical)?;

        progress("Building symmetric orthogonalizer");
        let step_start = Instant::now();
        let s_inv_sqrt =
            Self::symmetric_orthogonalizer(&overlap_matrix).map_err(ScfSetupError::Numerical)?;
        setup_timings.orthogonalizer = step_start.elapsed();

        progress("Building electron repulsion integrals");
        let step_start = Instant::now();
        let two_electron_integrals = electron_repulsion_ints(basis);
        setup_timings.electron_repulsion_integrals = step_start.elapsed();

        progress("Building initial density guess");
        let step_start = Instant::now();
        let total_density = density_guess_builder
            .build_density_guess(&h_core, molecule, basis)
            .map_err(ScfSetupError::DensityGuess)?;
        let density =
            split_density_guess(total_density, molecule.total_electrons(), occupied_orbitals);
        setup_timings.density_guess = step_start.elapsed();

        progress("Building initial molecular orbitals");
        let step_start = Instant::now();
        let (mo_coefficients, _) = Self::initial_mo_coefficients(&h_core, &s_inv_sqrt)
            .map_err(ScfSetupError::Numerical)?;
        setup_timings.initial_orbitals = step_start.elapsed();

        let fock = SpinMatrices {
            alpha: h_core.clone(),
            beta: h_core.clone(),
        };
        setup_timings.total = setup_start.elapsed();

        Ok(Self {
            molecule,
            basis,
            max_iterations,
            convergence_threshold,
            energy: 0.0,
            mo_coefficients: SpinMatrices::duplicate(mo_coefficients),
            density,
            fock,
            residual_norm: f64::INFINITY,
            diis: None,
            two_electron_integrals,
            h_core,
            t_matrix,
            v_matrix,
            overlap_matrix,
            s_inv_sqrt,
            occupied_orbitals,
            timings: ScfTimings {
                setup: setup_timings,
                ..ScfTimings::default()
            },
        })
    }

    pub fn enable_diis(&mut self, diis_size: usize) -> Result<(), DiisError> {
        self.diis = Some(SpinDiisAccelerators::duplicate(DiisAccelerator::try_new(
            diis_size,
        )?));
        Ok(())
    }

    #[allow(dead_code)]
    pub fn run(&mut self) -> Result<ScfResult, NumericalError> {
        let mut observer = NoopScfObserver;
        self.run_with_observer(&mut observer)
    }

    pub fn run_with_observer<O>(&mut self, observer: &mut O) -> Result<ScfResult, NumericalError>
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

            if i == 0 {
                self.update_fock_matrices();
            }
            self.apply_diis_if_enabled();
            self.solve_roothaan_hall_equations()?;
            self.update_density_matrices();
            self.update_residual_norm_and_next_fock();
            self.update_total_energy_from_current_fock();

            ensure_finite_value(self.energy, "UHF electronic energy")?;
            ensure_finite_value(self.residual_norm, "UHF residual norm")?;
            delta_energy = (self.energy - energy_last).abs();
            ensure_finite_value(delta_energy, "UHF delta energy")?;

            observer.on_iteration(&ScfIteration {
                iteration: iterations,
                electronic_energy: self.energy,
                delta_energy,
                residual_norm: self.residual_norm,
            });

            if delta_energy < self.convergence_threshold
                && self.residual_norm < self.convergence_threshold
            {
                converged = true;
                break;
            }

            energy_last = self.energy;
        }
        self.timings.iterations = iterations_start.elapsed();

        let nuclear_repulsion = self.molecule.geometry.nucl_repulsion();
        let total_energy = self.energy + nuclear_repulsion;
        let final_energy_details_start = Instant::now();
        let energy_details = self.calculate_energy_details();
        self.timings.final_energy_details = final_energy_details_start.elapsed();
        self.timings.total = run_start.elapsed() + self.timings.setup.total;

        Ok(ScfResult {
            converged,
            iterations,
            electronic_energy: self.energy,
            nuclear_repulsion_energy: nuclear_repulsion,
            total_energy,
            delta_energy,
            residual_norm: self.residual_norm,
            energy_details,
            timings: self.timings.clone(),
        })
    }

    fn initial_mo_coefficients(
        h_core: &DMatrix<f64>,
        s_inv_sqrt: &DMatrix<f64>,
    ) -> Result<(DMatrix<f64>, DVector<f64>), NumericalError> {
        let fock_preconditioned = &s_inv_sqrt.transpose() * h_core * s_inv_sqrt;
        let eig = fock_preconditioned.symmetric_eigen();
        let mo_coefficients = s_inv_sqrt * eig.eigenvectors;
        Self::sort_orbitals(mo_coefficients, eig.eigenvalues)
    }

    fn sort_orbitals(
        mo_coefficients: DMatrix<f64>,
        orbital_energies: DVector<f64>,
    ) -> Result<(DMatrix<f64>, DVector<f64>), NumericalError> {
        ensure_finite_values(&orbital_energies, "orbital energies")?;
        let mut order: Vec<usize> = (0..orbital_energies.len()).collect();
        order.sort_by(|&a, &b| orbital_energies[a].total_cmp(&orbital_energies[b]));

        let sorted_vectors = order
            .iter()
            .map(|&i| mo_coefficients.column(i).into_owned())
            .collect::<Vec<_>>();
        let sorted_energies =
            DVector::from_iterator(order.len(), order.iter().map(|&i| orbital_energies[i]));

        Ok((DMatrix::from_columns(&sorted_vectors), sorted_energies))
    }

    fn symmetric_orthogonalizer(s: &DMatrix<f64>) -> Result<DMatrix<f64>, NumericalError> {
        ensure_positive_definite(s, "overlap")?;
        let eig = s.clone().symmetric_eigen();
        let inv_sqrt_values = eig.eigenvalues.map(|value| 1.0 / value.sqrt());
        Ok(&eig.eigenvectors
            * DMatrix::from_diagonal(&inv_sqrt_values)
            * eig.eigenvectors.transpose())
    }

    fn update_fock_matrices(&mut self) {
        self.fock = self.build_fock_matrices(&self.density);
    }

    fn apply_diis_if_enabled(&mut self) {
        if let Some(diis) = &mut self.diis {
            if let Some(fock_matrix) =
                diis.alpha
                    .extrapolate(&self.fock.alpha, &self.density.alpha, &self.overlap_matrix)
            {
                self.fock.alpha = fock_matrix;
            }
            if let Some(fock_matrix) =
                diis.beta
                    .extrapolate(&self.fock.beta, &self.density.beta, &self.overlap_matrix)
            {
                self.fock.beta = fock_matrix;
            }
        }
    }

    fn solve_roothaan_hall_equations(&mut self) -> Result<(), NumericalError> {
        self.mo_coefficients = SpinMatrices::new(
            self.solve_roothaan_hall(&self.fock.alpha)?.0,
            self.solve_roothaan_hall(&self.fock.beta)?.0,
        );
        Ok(())
    }

    fn solve_roothaan_hall(
        &self,
        fock_matrix: &DMatrix<f64>,
    ) -> Result<(DMatrix<f64>, DVector<f64>), NumericalError> {
        let fock_preconditioned = &self.s_inv_sqrt.transpose() * fock_matrix * &self.s_inv_sqrt;
        let eig = fock_preconditioned.symmetric_eigen();
        let mo_coefficients = &self.s_inv_sqrt * eig.eigenvectors;
        Self::sort_orbitals(mo_coefficients, eig.eigenvalues)
    }

    fn update_density_matrices(&mut self) {
        self.density = self.mo_coefficients.clone().zip_map(
            self.occupied_orbitals,
            |mo_coefficients, occupied_orbitals| {
                density_from_mo_coefficients(&mo_coefficients, occupied_orbitals)
            },
        );
    }

    fn update_residual_norm_and_next_fock(&mut self) {
        let next_fock = self.build_fock_matrices(&self.density);
        let alpha_residual =
            scf_residual_norm(&next_fock.alpha, &self.density.alpha, &self.overlap_matrix);
        let beta_residual =
            scf_residual_norm(&next_fock.beta, &self.density.beta, &self.overlap_matrix);
        self.residual_norm = alpha_residual.hypot(beta_residual);
        self.fock = next_fock;
    }

    fn build_fock_matrices(&self, density: &SpinMatrices) -> SpinMatrices {
        let total_density = &density.alpha + &density.beta;
        SpinMatrices {
            alpha: self.build_spin_fock_matrix(&total_density, &density.alpha),
            beta: self.build_spin_fock_matrix(&total_density, &density.beta),
        }
    }

    fn build_spin_fock_matrix(
        &self,
        total_density: &DMatrix<f64>,
        same_spin_density: &DMatrix<f64>,
    ) -> DMatrix<f64> {
        let nbasis = self.basis.nbasis();
        let n_pairs = nbasis * (nbasis + 1) / 2;
        let values = (0..n_pairs)
            .into_par_iter()
            .map(|index| {
                let (mu, nu) = basis_function_pair(index);
                let mut g_term = 0.0;

                for lambda in 0..nbasis {
                    for sigma in 0..nbasis {
                        let coulomb_term = self.two_electron_integrals[(mu, nu, lambda, sigma)];
                        let exchange_term = self.two_electron_integrals[(mu, sigma, lambda, nu)];
                        g_term += total_density[(lambda, sigma)] * coulomb_term
                            - same_spin_density[(lambda, sigma)] * exchange_term;
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

    fn update_total_energy_from_current_fock(&mut self) {
        self.energy = self.calculate_total_energy_from_current_fock();
    }

    fn calculate_total_energy_from_current_fock(&self) -> f64 {
        0.5 * self.density.alpha.dot(&(&self.h_core + &self.fock.alpha))
            + 0.5 * self.density.beta.dot(&(&self.h_core + &self.fock.beta))
    }

    fn calculate_energy_details(&self) -> ScfEnergyDetails {
        let total_density = &self.density.alpha + &self.density.beta;
        let ((kinetic_energy, nuclear_attraction_energy), electron_repulsion_energy) = rayon::join(
            || {
                rayon::join(
                    || total_density.dot(&self.t_matrix),
                    || total_density.dot(&self.v_matrix),
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

    fn calculate_electron_repulsion_energy(&self) -> f64 {
        let total_density = &self.density.alpha + &self.density.beta;
        let nbasis = self.basis.nbasis();
        let e_re: f64 = (0..nbasis.pow(4))
            .into_par_iter()
            .map(|index| {
                let mu = index % nbasis;
                let nu = (index / nbasis) % nbasis;
                let lambda = (index / nbasis.pow(2)) % nbasis;
                let sigma = index / nbasis.pow(3);
                let coulomb_term = self.two_electron_integrals[(mu, nu, lambda, sigma)];
                let exchange_term = self.two_electron_integrals[(mu, sigma, lambda, nu)];

                total_density[(mu, nu)] * total_density[(lambda, sigma)] * coulomb_term
                    - self.density.alpha[(mu, nu)]
                        * self.density.alpha[(lambda, sigma)]
                        * exchange_term
                    - self.density.beta[(mu, nu)]
                        * self.density.beta[(lambda, sigma)]
                        * exchange_term
            })
            .sum();
        0.5 * e_re
    }
}

fn alpha_beta_occupied_orbitals<E>(molecule: &Molecule) -> Result<Spin<usize>, UhfSetupError<E>>
where
    E: std::error::Error + 'static,
{
    let electrons = molecule.total_electrons();
    let spin = molecule.unpaired_electrons() as usize;
    if spin > electrons || (electrons + spin) % 2 != 0 {
        return Err(UhfSetupError::InvalidElectronConfiguration {
            electrons,
            multiplicity: molecule.multiplicity.get(),
        });
    }
    let alpha = ((electrons + spin) / 2) as usize;
    let beta = ((electrons - spin) / 2) as usize;
    Ok(Spin::new(alpha, beta))
}

fn split_density_guess(
    total_density: DMatrix<f64>,
    electrons: usize,
    occupied_orbitals: Spin<usize>,
) -> SpinMatrices {
    if electrons == 0 {
        return SpinMatrices {
            alpha: DMatrix::zeros(total_density.nrows(), total_density.ncols()),
            beta: DMatrix::zeros(total_density.nrows(), total_density.ncols()),
        };
    }
    let alpha_scale = occupied_orbitals.alpha as f64 / electrons as f64;
    let beta_scale = occupied_orbitals.beta as f64 / electrons as f64;
    SpinMatrices {
        alpha: &total_density * alpha_scale,
        beta: total_density * beta_scale,
    }
}

fn density_from_mo_coefficients(
    mo_coefficients: &DMatrix<f64>,
    occupied_orbitals: usize,
) -> DMatrix<f64> {
    let c_occ = mo_coefficients.columns(0, occupied_orbitals);
    c_occ * c_occ.transpose()
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

fn basis_function_pair(pair_index: usize) -> (usize, usize) {
    let first = (((8 * pair_index + 1) as f64).sqrt() as usize - 1) / 2;
    let second = pair_index - first * (first + 1) / 2;
    (first, second)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        hf::density_guess::{core_hamiltonian::CoreHamiltonian, one_electron::OneElectron},
        test_utils,
    };
    use approx::assert_abs_diff_eq;

    #[test]
    fn test_uhf_h2_singlet_matches_rhf_reference_energy() {
        const PYSCF_RHF_ELECTRONIC_ENERGY: f64 = -1.831863646477507;
        const PYSCF_RHF_TOTAL_ENERGY: f64 = -1.116759307396425;

        let geometry = test_utils::load_sample_geometry_in_bohr("samples/h2/molecule.xyz");
        let basis = test_utils::load_sto3g_basis(&geometry);
        let molecule = Molecule::from(geometry);
        let mut uhf =
            UhfCalculation::new(&molecule, &basis, 100, 1e-8, OneElectron::default()).unwrap();

        let result = uhf.run().unwrap();

        assert!(result.converged);
        assert_abs_diff_eq!(
            result.electronic_energy,
            PYSCF_RHF_ELECTRONIC_ENERGY,
            epsilon = 1e-8
        );
        assert_abs_diff_eq!(result.total_energy, PYSCF_RHF_TOTAL_ENERGY, epsilon = 1e-8);
    }

    #[test]
    fn test_uhf_spin_density_electron_counts_match_multiplicity() {
        let geometry = test_utils::load_sample_geometry_in_bohr("samples/h2/molecule.xyz");
        let basis = test_utils::load_sto3g_basis(&geometry);
        // SAFETY: H2+ has one electron.
        let molecule = unsafe {
            Molecule::new_unchecked(
                geometry,
                crate::molecules::units::Units::Bohr,
                1,
                std::num::NonZeroU8::new(2).unwrap(),
            )
        };
        let mut uhf =
            UhfCalculation::new(&molecule, &basis, 100, 1e-8, OneElectron::default()).unwrap();

        let result = uhf.run().unwrap();
        let overlap = basis.overlap_ints();
        let alpha_electrons = (&uhf.density.alpha * &overlap).trace();
        let beta_electrons = (&uhf.density.beta * &overlap).trace();

        assert!(result.converged);
        assert_abs_diff_eq!(alpha_electrons, 1.0, epsilon = 1e-8);
        assert_abs_diff_eq!(beta_electrons, 0.0, epsilon = 1e-8);
        assert_abs_diff_eq!(
            alpha_electrons - beta_electrons,
            molecule.unpaired_electrons() as f64,
            epsilon = 1e-8
        );
    }

    #[test]
    fn test_uhf_oh_doublet_matches_pyscf_reference_energy() {
        const PYSCF_UHF_ELECTRONIC_ENERGY: f64 = -78.727017326066203;
        const PYSCF_UHF_TOTAL_ENERGY: f64 = -74.362669194767236;

        let geometry = test_utils::load_sample_geometry_in_bohr("samples/oh/oh.xyz");
        let basis = test_utils::load_sto3g_basis(&geometry);
        // SAFETY: Neutral OH has nine electrons.
        let molecule = unsafe {
            Molecule::new_unchecked(
                geometry,
                crate::molecules::units::Units::Bohr,
                0,
                std::num::NonZeroU8::new(2).unwrap(),
            )
        };
        let mut uhf =
            UhfCalculation::new(&molecule, &basis, 100, 1e-5, CoreHamiltonian::default()).unwrap();
        uhf.enable_diis(6).unwrap();

        let result = uhf.run().unwrap();
        let overlap = basis.overlap_ints();
        let alpha_electrons = (&uhf.density.alpha * &overlap).trace();
        let beta_electrons = (&uhf.density.beta * &overlap).trace();

        assert!(result.converged);
        assert_abs_diff_eq!(
            result.electronic_energy,
            PYSCF_UHF_ELECTRONIC_ENERGY,
            epsilon = 5e-7
        );
        assert_abs_diff_eq!(result.total_energy, PYSCF_UHF_TOTAL_ENERGY, epsilon = 5e-7);
        assert_abs_diff_eq!(alpha_electrons, 5.0, epsilon = 1e-8);
        assert_abs_diff_eq!(beta_electrons, 4.0, epsilon = 1e-8);
    }

    #[test]
    fn test_uhf_rejects_incompatible_multiplicity() {
        let geometry = test_utils::load_sample_geometry_in_bohr("samples/h2/molecule.xyz");
        let basis = test_utils::load_sto3g_basis(&geometry);
        // SAFETY: Neutral H2 has two electrons.
        let molecule = unsafe {
            Molecule::new_unchecked(
                geometry,
                crate::molecules::units::Units::Bohr,
                0,
                std::num::NonZeroU8::new(2).unwrap(),
            )
        };

        let error = match UhfCalculation::new(&molecule, &basis, 10, 1e-8, OneElectron::default()) {
            Ok(_) => panic!("expected incompatible multiplicity to be rejected"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            UhfSetupError::InvalidElectronConfiguration {
                electrons: 2,
                multiplicity: 2
            }
        ));
    }
}
