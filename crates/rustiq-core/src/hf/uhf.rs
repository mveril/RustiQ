use std::time::Instant;

use nalgebra::{DMatrix, DVector};
use rayon::prelude::*;
use thiserror::Error;

use crate::{
    basis::gaussian::basis::Basis,
    config::DiisConfig,
    eri::{index::PairIndex, CompactEri, EriError},
    hf::numerical_error::{ensure_finite_value, ensure_finite_values, NumericalError},
    molecules::Molecule,
};

use super::orthogonalization::OrthogonalizationInfo;
use super::{
    density_guess::{mo_coefficients_from_fock_like_matrix, DensityGuess, OrbitalGuess},
    diis::{DiisAccelerator, DiisError},
    scf::ScfSetupError,
    scf_energy_details::ScfEnergyDetails,
    scf_iteration::ScfIteration,
    scf_result::{ScfOutcome, ScfResult, ScfTermination, ScfTimings, SpinDiagnostics},
    scf_setup::{prepare_scf_setup, PreparedScfSetup, ScfPreparationError, ScfSetupStep},
};

#[derive(Debug, Error)]
pub enum UhfSetupError<E>
where
    E: std::error::Error + 'static,
{
    #[error(transparent)]
    Scf(#[from] ScfSetupError<E>),
    #[error(transparent)]
    ElectronRepulsion(#[from] EriError),
}

pub(crate) use super::spin::Spin;

pub(crate) type SpinMatrices = Spin<DMatrix<f64>>;
pub struct UhfCalculation<'a> {
    pub molecule: &'a Molecule,
    pub basis: &'a Basis,
    pub max_iterations: usize,
    pub convergence_threshold: f64,
    pub energy: f64,
    pub mo_coefficients: SpinMatrices,
    pub orbital_energies: Spin<DVector<f64>>,
    pub density: SpinMatrices,
    /// False only for the Zero startup sentinel; orbital updates establish Nα/Nβ.
    density_has_required_population: bool,
    pub fock: SpinMatrices,
    pub residual_norm: f64,
    diis: Option<DiisAccelerator>,
    pub two_electron_integrals: CompactEri,
    pub h_core: DMatrix<f64>,
    pub t_matrix: DMatrix<f64>,
    pub v_matrix: DMatrix<f64>,
    overlap_matrix: DMatrix<f64>,
    orthogonalizer: DMatrix<f64>,
    orthogonalization: OrthogonalizationInfo,
    pub occupied_orbitals: Spin<usize>,
    timings: ScfTimings,
}

impl<'a> UhfCalculation<'a> {
    #[allow(
        dead_code,
        reason = "Retained scientific helpers and representations support tests, benchmarks, or future internal use"
    )]
    pub fn new<G>(
        molecule: &'a Molecule,
        basis: &'a Basis,
        max_iterations: usize,
        convergence_threshold: f64,
        linear_dependency_threshold: f64,
        density_guess_builder: G,
    ) -> Result<Self, UhfSetupError<G::Error>>
    where
        G: DensityGuess<super::reference::Unrestricted>,
        G::Error: 'static,
    {
        Self::new_with_progress(
            molecule,
            basis,
            max_iterations,
            convergence_threshold,
            linear_dependency_threshold,
            density_guess_builder,
            |_| {},
        )
    }

    pub fn new_with_progress<G, F>(
        molecule: &'a Molecule,
        basis: &'a Basis,
        max_iterations: usize,
        convergence_threshold: f64,
        linear_dependency_threshold: f64,
        density_guess_builder: G,
        mut progress: F,
    ) -> Result<Self, UhfSetupError<G::Error>>
    where
        G: DensityGuess<super::reference::Unrestricted>,
        G::Error: 'static,
        F: FnMut(ScfSetupStep),
    {
        let occupied_orbitals = alpha_beta_occupied_orbitals(molecule);
        let required_occupied_orbitals = occupied_orbitals.alpha.max(occupied_orbitals.beta);
        let prepared = prepare_scf_setup(
            molecule,
            basis,
            required_occupied_orbitals,
            linear_dependency_threshold,
            &mut progress,
        )
        .map_err(|error| match error {
            ScfPreparationError::ElectronRepulsion(error) => {
                UhfSetupError::ElectronRepulsion(error)
            }
            ScfPreparationError::Numerical(error) => {
                UhfSetupError::Scf(ScfSetupError::Numerical(error))
            }
        })?;
        Self::new_with_prepared(
            molecule,
            basis,
            max_iterations,
            convergence_threshold,
            density_guess_builder,
            prepared,
            progress,
        )
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "Scientific setup requires these distinct inputs; keeping the existing interface avoids a broader refactor"
    )]
    #[allow(
        clippy::needless_pass_by_value,
        reason = "All density-guess strategies share an owned setup interface"
    )]
    pub(crate) fn new_with_prepared<G, F>(
        molecule: &'a Molecule,
        basis: &'a Basis,
        max_iterations: usize,
        convergence_threshold: f64,
        density_guess_builder: G,
        prepared: PreparedScfSetup,
        mut progress: F,
    ) -> Result<Self, UhfSetupError<G::Error>>
    where
        G: DensityGuess<super::reference::Unrestricted>,
        G::Error: 'static,
        F: FnMut(ScfSetupStep),
    {
        let occupied_orbitals = alpha_beta_occupied_orbitals(molecule);
        let setup_start = Instant::now();
        let PreparedScfSetup {
            integrals,
            orthogonalizer,
            orthogonalization,
            mut timings,
        } = prepared;
        let super::integrals::ScfIntegrals {
            overlap: overlap_matrix,
            kinetic: t_matrix,
            nuclear_attraction: v_matrix,
            electron_repulsion: two_electron_integrals,
        } = integrals;
        let h_core = &t_matrix + &v_matrix;

        progress(ScfSetupStep::InitialDensityGuess);
        let step_start = Instant::now();
        let orbital_guess = density_guess_builder
            .build_orbital_guess(&h_core, basis)
            .map_err(ScfSetupError::DensityGuess)?;
        let density_has_required_population = !matches!(&orbital_guess, OrbitalGuess::Zero);
        let (density, mo_coefficients, orbital_energies) = match orbital_guess {
            OrbitalGuess::FockLike(Spin { alpha, beta }) => {
                let mo_coefficients = SpinMatrices::new(
                    mo_coefficients_from_fock_like_matrix(&alpha, &orthogonalizer)
                        .map_err(ScfSetupError::Numerical)?,
                    mo_coefficients_from_fock_like_matrix(&beta, &orthogonalizer)
                        .map_err(ScfSetupError::Numerical)?,
                );
                let density = mo_coefficients
                    .clone()
                    .zip_map(occupied_orbitals, |coefficients, occupied| {
                        density_from_mo_coefficients(&coefficients, occupied)
                    });
                let orbital_energies = Spin::new(
                    DVector::zeros(mo_coefficients.alpha.ncols()),
                    DVector::zeros(mo_coefficients.beta.ncols()),
                );
                (density, mo_coefficients, orbital_energies)
            }
            OrbitalGuess::Zero => (
                SpinMatrices::duplicate(DMatrix::zeros(basis.nbasis(), basis.nbasis())),
                SpinMatrices::duplicate(DMatrix::zeros(basis.nbasis(), orthogonalizer.ncols())),
                Spin::duplicate(DVector::zeros(orthogonalizer.ncols())),
            ),
        };
        timings.density_guess = step_start.elapsed();

        let fock = SpinMatrices {
            alpha: h_core.clone(),
            beta: h_core.clone(),
        };
        timings.total += setup_start.elapsed();

        Ok(Self {
            molecule,
            basis,
            max_iterations,
            convergence_threshold,
            energy: 0.0,
            mo_coefficients,
            orbital_energies,
            density,
            density_has_required_population,
            fock,
            residual_norm: f64::INFINITY,
            diis: None,
            two_electron_integrals,
            h_core,
            t_matrix,
            v_matrix,
            overlap_matrix,
            orthogonalizer,
            orthogonalization,
            occupied_orbitals,
            timings: ScfTimings {
                setup: timings,
                ..ScfTimings::default()
            },
        })
    }

    pub fn enable_diis(&mut self, diis_size: usize) -> Result<(), DiisError> {
        self.diis = Some(DiisAccelerator::try_new(diis_size)?);
        Ok(())
    }

    pub(crate) fn configure_diis(&mut self, config: &DiisConfig) {
        self.diis = config
            .enabled
            .then(|| DiisAccelerator::new(config.max_history.value));
    }

    #[allow(
        dead_code,
        reason = "Retained helper supports scientific tests and benchmarks"
    )]
    pub fn run(&mut self) -> Result<ScfOutcome, NumericalError> {
        self.run_with_iterations(|_| {})
    }

    pub fn run_with_iterations<O>(&mut self, mut observer: O) -> Result<ScfOutcome, NumericalError>
    where
        O: FnMut(&ScfIteration),
    {
        let mut energy_last = 0.0;
        let mut termination = ScfTermination::Unconverged;
        let mut iterations = 0;
        let mut delta_energy = f64::INFINITY;
        let run_start = Instant::now();
        let iterations_start = Instant::now();

        let mut density_matches_orbitals = true;
        for i in 0..self.max_iterations {
            iterations = i + 1;

            if i == 0 {
                self.update_fock_matrices();
            }
            // Preserve ODA for ordinary uphill steps as well as DIIS proposals,
            // but never interpolate the Zero sentinel with populated densities.
            let previous_state = self
                .diis
                .as_ref()
                .filter(|_| self.density_has_required_population)
                .map(|_| {
                    (
                        self.density.clone(),
                        self.fock.clone(),
                        self.calculate_total_energy_from_current_fock(),
                    )
                });
            let diis_extrapolated = self.apply_diis_if_enabled();
            self.solve_roothaan_hall_equations()?;
            self.update_density_matrices();
            self.update_residual_norm_and_next_fock();
            self.update_total_energy_from_current_fock();

            density_matches_orbitals = true;
            if let Some((previous_density, previous_fock, previous_energy)) = previous_state {
                let roundoff = 64.0 * f64::EPSILON * previous_energy.abs().max(1.0);
                if self.energy > previous_energy + roundoff {
                    self.diis
                        .as_mut()
                        .expect("DIIS is enabled during energy safeguarding")
                        .clear();
                    if diis_extrapolated {
                        self.fock = previous_fock.clone();
                        self.solve_roothaan_hall_equations()?;
                        self.update_density_matrices();
                        self.update_residual_norm_and_next_fock();
                    }
                    let fraction = self.apply_optimal_damping(&previous_density, &previous_fock)?;
                    #[allow(clippy::float_cmp, reason = "Exactly one marks the undamped endpoint")]
                    {
                        density_matches_orbitals = fraction == 1.0;
                    }
                }
            }

            ensure_finite_value(self.energy, "UHF electronic energy")?;
            ensure_finite_value(self.residual_norm, "UHF residual norm")?;
            delta_energy = (self.energy - energy_last).abs();
            ensure_finite_value(delta_energy, "UHF delta energy")?;

            observer(&ScfIteration {
                iteration: iterations,
                electronic_energy: self.energy,
                delta_energy,
                residual_norm: self.residual_norm,
            });

            if density_matches_orbitals
                && delta_energy < self.convergence_threshold
                && self.residual_norm < self.convergence_threshold
            {
                termination = ScfTermination::Converged;
                break;
            }

            energy_last = self.energy;
        }
        if matches!(termination, ScfTermination::Converged) {
            delta_energy =
                self.canonicalize_final_fock(delta_energy, &mut iterations, &mut observer)?;
        }
        self.timings.iterations = iterations_start.elapsed();

        let nuclear_repulsion = self.molecule.geometry().nucl_repulsion()?;
        let total_energy = self.energy + nuclear_repulsion;
        let final_energy_details_start = Instant::now();
        let energy_details = self.calculate_energy_details();
        // Interior mixtures and the previous endpoint do not match the current
        // candidate orbitals, so those orbitals cannot supply spin diagnostics.
        let spin = density_matches_orbitals
            .then(|| self.spin_diagnostics())
            .transpose()?;
        self.timings.final_energy_details = final_energy_details_start.elapsed();
        self.timings.total = run_start.elapsed() + self.timings.setup.total;

        Ok(termination.with_result(ScfResult {
            iterations,
            electronic_energy: self.energy,
            nuclear_repulsion_energy: nuclear_repulsion,
            total_energy,
            delta_energy,
            residual_norm: self.residual_norm,
            spin,
            energy_details,
            orthogonalization: self.orthogonalization,
            timings: self.timings.clone(),
        }))
    }

    #[allow(
        clippy::cast_precision_loss,
        reason = "Allocated orbital counts are represented in f64 for the spin expectation values"
    )]
    fn spin_diagnostics(&self) -> Result<SpinDiagnostics, NumericalError> {
        let n_alpha = self.occupied_orbitals.alpha;
        let n_beta = self.occupied_orbitals.beta;
        let occupied_overlap = self.mo_coefficients.alpha.columns(0, n_alpha).transpose()
            * &self.overlap_matrix
            * self.mo_coefficients.beta.columns(0, n_beta);
        let spin_z = (n_alpha as f64 - n_beta as f64) * 0.5;
        let ideal_s_squared = spin_z * (spin_z + 1.0);
        // The alpha-beta occupied-orbital overlap must include the AO metric.
        let s_squared =
            spin_z * spin_z + (n_alpha + n_beta) as f64 * 0.5 - occupied_overlap.norm_squared();
        let spin_contamination = s_squared - ideal_s_squared;
        ensure_finite_value(s_squared, "UHF spin expectation")?;
        ensure_finite_value(spin_contamination, "UHF spin contamination")?;
        Ok(SpinDiagnostics {
            s_squared,
            ideal_s_squared,
            spin_contamination,
        })
    }

    fn sort_orbitals(
        mo_coefficients: &DMatrix<f64>,
        orbital_energies: &DVector<f64>,
    ) -> Result<(DMatrix<f64>, DVector<f64>), NumericalError> {
        ensure_finite_values(orbital_energies, "orbital energies")?;
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

    fn update_fock_matrices(&mut self) {
        self.fock = self.build_fock_matrices(&self.density);
    }

    fn apply_optimal_damping(
        &mut self,
        previous_density: &SpinMatrices,
        previous_fock: &SpinMatrices,
    ) -> Result<f64, NumericalError> {
        let segments = Spin::new(
            super::damping::DensitySegment {
                previous_density: &previous_density.alpha,
                density: &self.density.alpha,
                previous_fock: &previous_fock.alpha,
                fock: &self.fock.alpha,
            },
            super::damping::DensitySegment {
                previous_density: &previous_density.beta,
                density: &self.density.beta,
                previous_fock: &previous_fock.beta,
                fock: &self.fock.beta,
            },
        );
        let fraction = super::damping::damping_fraction(segments)?;
        if fraction == 0.0 {
            self.density = previous_density.clone();
            self.fock = previous_fock.clone();
        } else if fraction < 1.0 {
            self.density = previous_density
                .as_ref()
                .zip_map(self.density.as_ref(), |old, new| {
                    old + (new - old) * fraction
                });
            self.fock = previous_fock
                .as_ref()
                .zip_map(self.fock.as_ref(), |old, new| old + (new - old) * fraction);
        }
        let (alpha_residual, beta_residual) = self
            .fock
            .as_ref()
            .zip_map(self.density.as_ref(), |fock, density| {
                scf_residual_norm(fock, density, &self.overlap_matrix)
            })
            .into_parts();
        self.residual_norm = alpha_residual.hypot(beta_residual);
        self.update_total_energy_from_current_fock();
        Ok(fraction)
    }

    /// Returns whether a genuine extrapolation replaced the physical Fock pair.
    fn apply_diis_if_enabled(&mut self) -> bool {
        if let Some(diis) = &mut self.diis {
            if let Some(fock) = diis.extrapolate_spin(
                &self.fock.as_ref(),
                &self.density.as_ref(),
                &self.overlap_matrix,
            ) {
                self.fock = fock;
                return true;
            }
        }
        false
    }

    fn solve_roothaan_hall_equations(&mut self) -> Result<(), NumericalError> {
        let (alpha_mo_coefficients, alpha_orbital_energies) =
            self.solve_roothaan_hall(&self.fock.alpha)?;
        let (beta_mo_coefficients, beta_orbital_energies) =
            self.solve_roothaan_hall(&self.fock.beta)?;
        self.mo_coefficients = SpinMatrices::new(alpha_mo_coefficients, beta_mo_coefficients);
        self.orbital_energies = Spin::new(alpha_orbital_energies, beta_orbital_energies);
        Ok(())
    }

    fn canonicalize_final_fock<O: FnMut(&ScfIteration)>(
        &mut self,
        mut delta_energy: f64,
        iterations: &mut usize,
        observer: &mut O,
    ) -> Result<f64, NumericalError> {
        // As in RHF, canonicalization must not expose orbitals whose occupied
        // projectors differ materially from the density used to build F.
        loop {
            self.solve_roothaan_hall_equations()?;
            let canonical_density = self.mo_coefficients.clone().zip_map(
                self.occupied_orbitals,
                |mo_coefficients, occupied_orbitals| {
                    density_from_mo_coefficients(&mo_coefficients, occupied_orbitals)
                },
            );
            let density_change = (&canonical_density.alpha - &self.density.alpha)
                .norm()
                .hypot((&canonical_density.beta - &self.density.beta).norm());

            if density_change < self.convergence_threshold
                && delta_energy < self.convergence_threshold
                && self.residual_norm < self.convergence_threshold
            {
                return Ok(delta_energy);
            }

            // A diagonalization that only verifies the final orbitals is not an
            // SCF update. Any further density update uses the shared iteration budget.
            if *iterations >= self.max_iterations {
                return Err(NumericalError::FinalizationNotConverged {
                    max_iterations: self.max_iterations,
                });
            }
            let previous_energy = self.energy;
            self.density = canonical_density;
            self.update_residual_norm_and_next_fock();
            self.update_total_energy_from_current_fock();
            ensure_finite_value(self.energy, "UHF electronic energy")?;
            ensure_finite_value(self.residual_norm, "UHF residual norm")?;
            delta_energy = (self.energy - previous_energy).abs();
            ensure_finite_value(delta_energy, "UHF delta energy")?;
            *iterations += 1;
            observer(&ScfIteration {
                iteration: *iterations,
                electronic_energy: self.energy,
                delta_energy,
                residual_norm: self.residual_norm,
            });
        }
    }

    fn solve_roothaan_hall(
        &self,
        fock_matrix: &DMatrix<f64>,
    ) -> Result<(DMatrix<f64>, DVector<f64>), NumericalError> {
        let fock_preconditioned =
            &self.orthogonalizer.transpose() * fock_matrix * &self.orthogonalizer;
        let eig = fock_preconditioned.symmetric_eigen();
        let mo_coefficients = &self.orthogonalizer * eig.eigenvectors;
        Self::sort_orbitals(&mo_coefficients, &eig.eigenvalues)
    }

    fn update_density_matrices(&mut self) {
        self.density = self.mo_coefficients.as_ref().zip_map(
            self.occupied_orbitals,
            |mo_coefficients, occupied_orbitals| {
                density_from_mo_coefficients(mo_coefficients, occupied_orbitals)
            },
        );
        self.density_has_required_population = true;
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
        let nbasis = self.basis.nbasis();
        let n_pairs = nbasis * (nbasis + 1) / 2;
        let values = (0..n_pairs)
            .into_par_iter()
            .map(|index| {
                let (mu, nu) = PairIndex(index).indices();
                let mut alpha = 0.0;
                let mut beta = 0.0;

                for lambda in 0..nbasis {
                    for sigma in 0..nbasis {
                        let coulomb = total_density[(lambda, sigma)]
                            * self.two_electron_integrals[(mu, nu, lambda, sigma)];
                        let exchange = self.two_electron_integrals[(mu, sigma, lambda, nu)];
                        // Share integral loads and Coulomb work, retaining each spin's
                        // exchange density and the established accumulation order.
                        alpha += coulomb - density.alpha[(lambda, sigma)] * exchange;
                        beta += coulomb - density.beta[(lambda, sigma)] * exchange;
                    }
                }

                (
                    mu,
                    nu,
                    self.h_core[(mu, nu)] + alpha,
                    self.h_core[(mu, nu)] + beta,
                )
            })
            .collect::<Vec<_>>();

        let mut fock = SpinMatrices {
            alpha: DMatrix::zeros(nbasis, nbasis),
            beta: DMatrix::zeros(nbasis, nbasis),
        };
        for (mu, nu, alpha, beta) in values {
            fock.alpha[(mu, nu)] = alpha;
            fock.beta[(mu, nu)] = beta;
            if mu != nu {
                fock.alpha[(nu, mu)] = alpha;
                fock.beta[(nu, mu)] = beta;
            }
        }
        fock
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
        let (kinetic_energy, nuclear_attraction_energy) = rayon::join(
            || total_density.dot(&self.t_matrix),
            || total_density.dot(&self.v_matrix),
        );
        let electron_repulsion_energy = self.energy - kinetic_energy - nuclear_attraction_energy;

        ScfEnergyDetails {
            kinetic_energy,
            nuclear_attraction_energy,
            electron_repulsion_energy,
        }
    }
}

pub(crate) fn alpha_beta_occupied_orbitals(molecule: &Molecule) -> Spin<usize> {
    let electrons = molecule.total_electrons();
    let spin = molecule.unpaired_electrons() as usize;
    let alpha = usize::midpoint(electrons, spin);
    let beta = (electrons - spin) / 2;
    Spin::new(alpha, beta)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{
            hf::GuessPerturbationConfig,
            random_config::{
                distribution_config::NormalDistributionConfig, DistributionConfig, RandomConfig,
            },
            validated::PositiveFiniteF64,
        },
        hf::density_guess::{core_hamiltonian::CoreHamiltonian, one_electron::OneElectron},
        test_utils,
    };
    use approx::assert_abs_diff_eq;

    #[test]
    fn damping_reference_coupled_uhf_matches_rebuilt_energy_on_segment() {
        let geometry = test_utils::load_sample_geometry_in_bohr("samples/oh/oh.xyz");
        let basis = test_utils::load_sto3g_basis(&geometry);
        let molecule = Molecule::try_new(
            geometry,
            crate::molecules::Units::Bohr,
            0,
            std::num::NonZeroU8::new(2).unwrap(),
        )
        .unwrap();
        let mut uhf =
            UhfCalculation::new(&molecule, &basis, 100, 1e-8, 1e-8, OneElectron::default())
                .unwrap();
        uhf.update_fock_matrices();
        let old_density = uhf.density.clone();
        let old_fock = uhf.fock.clone();
        let old_energy = uhf.calculate_total_energy_from_current_fock();
        uhf.solve_roothaan_hall_equations().unwrap();
        uhf.update_density_matrices();
        uhf.update_fock_matrices();
        let candidate = uhf.density.clone();
        let candidate_energy = uhf.calculate_total_energy_from_current_fock();
        for density in [&old_density, &candidate] {
            assert_abs_diff_eq!(
                (&density.alpha * &uhf.overlap_matrix).trace(),
                5.0,
                epsilon = 1e-10
            );
            assert_abs_diff_eq!(
                (&density.beta * &uhf.overlap_matrix).trace(),
                4.0,
                epsilon = 1e-10
            );
        }
        let da = &candidate.alpha - &old_density.alpha;
        let db = &candidate.beta - &old_density.beta;
        let slope = da.dot(&old_fock.alpha) + db.dot(&old_fock.beta);
        let curvature = f64::midpoint(
            da.dot(&(&uhf.fock.alpha - &old_fock.alpha)),
            db.dot(&(&uhf.fock.beta - &old_fock.beta)),
        );
        let fraction = uhf.apply_optimal_damping(&old_density, &old_fock).unwrap();
        assert_abs_diff_eq!(
            uhf.density.alpha.dot(&uhf.overlap_matrix),
            old_density.alpha.dot(&uhf.overlap_matrix),
            epsilon = 1e-10
        );
        assert_abs_diff_eq!(
            uhf.density.beta.dot(&uhf.overlap_matrix),
            old_density.beta.dot(&uhf.overlap_matrix),
            epsilon = 1e-10
        );
        assert!((0.0..=1.0).contains(&fraction));
        test_utils::write_damping_reference(
            "oh-uhf",
            &serde_json::json!({
            "system": "oh", "method": "uhf", "nao": basis.nbasis(),
            "previous": [old_density.alpha.as_slice(), old_density.beta.as_slice()],
            "candidate": [candidate.alpha.as_slice(), candidate.beta.as_slice()],
            "density": [uhf.density.alpha.as_slice(), uhf.density.beta.as_slice()],
            "fock": [uhf.fock.alpha.as_slice(), uhf.fock.beta.as_slice()],
            "energy": uhf.energy, "previous_energy": old_energy,
            "candidate_energy": candidate_energy, "fraction": fraction, "residual": uhf.residual_norm}),
        );
        let minimum = uhf.energy;
        for i in 0..=100 {
            let t = f64::from(i) / 100.0;
            uhf.density = old_density
                .as_ref()
                .zip_map(candidate.as_ref(), |old, new| old + (new - old) * t);
            uhf.update_fock_matrices();
            let actual = uhf.calculate_total_energy_from_current_fock();
            assert_abs_diff_eq!(
                actual,
                old_energy + slope * t + curvature * t * t,
                epsilon = 1e-10
            );
            assert!(minimum <= actual + 1e-10);
        }
    }

    #[test]
    fn uphill_diis_is_rejected_and_uhf_recovers() {
        let geometry = test_utils::load_sample_geometry_in_bohr("samples/oh/oh.xyz");
        let basis = test_utils::load_sto3g_basis(&geometry);
        let molecule = Molecule::try_new(
            geometry,
            crate::molecules::Units::Bohr,
            0,
            std::num::NonZeroU8::new(2).unwrap(),
        )
        .unwrap();
        let mut uhf =
            UhfCalculation::new(&molecule, &basis, 1, 1e-8, 1e-8, OneElectron::default()).unwrap();
        uhf.update_fock_matrices();
        let previous_energy = uhf.calculate_total_energy_from_current_fock();
        let previous_density = uhf.density.clone();
        let n = basis.nbasis();
        let mut bad_fock = DMatrix::zeros(n, 2 * n);
        bad_fock.columns_mut(0, n).copy_from(&(-&uhf.fock.alpha));
        bad_fock.columns_mut(n, n).copy_from(&(-&uhf.fock.beta));
        uhf.fock.alpha = bad_fock.columns(0, n).into_owned();
        uhf.fock.beta = bad_fock.columns(n, n).into_owned();
        uhf.solve_roothaan_hall_equations().unwrap();
        uhf.update_density_matrices();
        uhf.update_fock_matrices();
        assert!(uhf.calculate_total_energy_from_current_fock() > previous_energy + 1.0);
        uhf.density = previous_density;
        uhf.enable_diis(6).unwrap();
        uhf.diis
            .as_mut()
            .unwrap()
            .extrapolate_with_error(&bad_fock, DMatrix::zeros(n, 2 * n));
        let first = uhf.run().unwrap();
        assert!(matches!(first, ScfOutcome::Unconverged(_)));
        assert!(first.summary().electronic_energy <= previous_energy + 1e-12);
        assert!(!uhf.apply_diis_if_enabled());
        let rebuilt = uhf.build_fock_matrices(&uhf.density);
        assert_abs_diff_eq!(uhf.fock.alpha, rebuilt.alpha, epsilon = 1e-12);
        assert_abs_diff_eq!(uhf.fock.beta, rebuilt.beta, epsilon = 1e-12);
        assert_abs_diff_eq!(
            uhf.density.alpha.dot(&uhf.overlap_matrix),
            5.0,
            epsilon = 1e-10
        );
        assert_abs_diff_eq!(
            uhf.density.beta.dot(&uhf.overlap_matrix),
            4.0,
            epsilon = 1e-10
        );
        uhf.max_iterations = 100;
        let recovered = uhf.run().unwrap();
        assert!(matches!(recovered, ScfOutcome::Converged(_)));
        let mut reference =
            UhfCalculation::new(&molecule, &basis, 100, 1e-8, 1e-8, OneElectron::default())
                .unwrap();
        let reference = reference.run().unwrap();
        assert_abs_diff_eq!(
            recovered.summary().electronic_energy,
            reference.summary().electronic_energy,
            epsilon = 1e-8
        );
        assert_final_densities_match_canonical_orbitals(&uhf, 1e-8);
    }

    #[rstest::rstest]
    #[case(false)]
    #[case(true)]
    fn damping_reference_uhf_uphill_step_ends_on_a_consistent_mixture(#[case] extrapolate: bool) {
        let (molecule, basis, coefficients) = test_utils::stretched_h2_rotated_orbitals();
        let mut uhf = UhfCalculation::new(
            &molecule,
            &basis,
            1,
            1e-10,
            1e-8,
            CoreHamiltonian::default(),
        )
        .unwrap();
        uhf.mo_coefficients = Spin::duplicate(coefficients);
        uhf.update_density_matrices();
        uhf.update_fock_matrices();
        let old_density = uhf.density.clone();
        let old_fock = uhf.fock.clone();
        let old_energy = uhf.calculate_total_energy_from_current_fock();
        uhf.solve_roothaan_hall_equations().unwrap();
        uhf.update_density_matrices();
        uhf.update_residual_norm_and_next_fock();
        let candidate = uhf.density.clone();
        let candidate_energy = uhf.calculate_total_energy_from_current_fock();
        assert!(candidate_energy > old_energy + 0.05);
        uhf.density = old_density.clone();
        uhf.enable_diis(6).unwrap();
        if extrapolate {
            // A zero-error entry receives all the weight and selects the
            // opposite Fock spectrum. Its uphill proposal must be rejected.
            let mut bad_fock = DMatrix::zeros(2, 4);
            bad_fock.columns_mut(0, 2).copy_from(&(-&old_fock.alpha));
            bad_fock.columns_mut(2, 2).copy_from(&(-&old_fock.beta));
            uhf.diis
                .as_mut()
                .unwrap()
                .extrapolate_with_error(&bad_fock, DMatrix::zeros(2, 4));
        }
        let result = uhf.run().unwrap();
        assert!(matches!(result, ScfOutcome::Unconverged(_)));
        assert!(result.summary().spin.is_none());
        // Independent PySCF/BSE STO-3G result; all energies here are electronic.
        assert_abs_diff_eq!(old_energy, -0.597_659_544_429_562, epsilon = 1e-10);
        assert_abs_diff_eq!(candidate_energy, -0.521_769_776_413_940, epsilon = 1e-10);
        assert_abs_diff_eq!(uhf.energy, -0.735_533_659_273_390, epsilon = 1e-10);
        let step = &candidate.alpha - &old_density.alpha;
        let fraction = (&uhf.density.alpha - &old_density.alpha).dot(&step) / step.norm_squared();
        assert_abs_diff_eq!(fraction, 0.445_402_193_469_286, epsilon = 1e-10);
        for (density, old, new) in [
            (&uhf.density.alpha, &old_density.alpha, &candidate.alpha),
            (&uhf.density.beta, &old_density.beta, &candidate.beta),
        ] {
            assert_abs_diff_eq!(density, &(old + (new - old) * fraction), epsilon = 1e-12);
            assert_abs_diff_eq!(
                (density * &uhf.overlap_matrix).trace(),
                1.0,
                epsilon = 1e-12
            );
            assert!((density * &uhf.overlap_matrix * density - density).norm() > 0.1);
        }
        let physical = uhf.build_fock_matrices(&uhf.density);
        assert_abs_diff_eq!(uhf.fock.alpha, physical.alpha, epsilon = 1e-12);
        assert_abs_diff_eq!(uhf.fock.beta, physical.beta, epsilon = 1e-12);
        assert_abs_diff_eq!(
            uhf.energy,
            uhf.calculate_total_energy_from_current_fock(),
            epsilon = 1e-14
        );
        let residual =
            scf_residual_norm(&physical.alpha, &uhf.density.alpha, &uhf.overlap_matrix).hypot(
                scf_residual_norm(&physical.beta, &uhf.density.beta, &uhf.overlap_matrix),
            );
        assert_abs_diff_eq!(uhf.residual_norm, residual, epsilon = 1e-12);
        test_utils::write_damping_reference(
            if extrapolate {
                "h2-uhf-diis"
            } else {
                "h2-uhf-ordinary"
            },
            &serde_json::json!({"system": "h2", "method": "uhf", "nao": 2,
                "previous": [old_density.alpha.as_slice(), old_density.beta.as_slice()],
                "candidate": [candidate.alpha.as_slice(), candidate.beta.as_slice()],
                "density": [uhf.density.alpha.as_slice(), uhf.density.beta.as_slice()],
                "fock": [uhf.fock.alpha.as_slice(), uhf.fock.beta.as_slice()],
                "energy": uhf.energy, "previous_energy": old_energy,
                "candidate_energy": candidate_energy, "fraction": fraction, "residual": uhf.residual_norm}),
        );
        // Clearing is observable: one subsequent history entry cannot extrapolate.
        assert!(!uhf.apply_diis_if_enabled());
    }

    #[test]
    fn damping_reference_uhf_zero_startup_preserves_population_even_if_energy_is_positive() {
        let geometry = test_utils::load_sample_geometry_in_bohr("samples/h2/molecule.xyz");
        let basis = test_utils::load_sto3g_basis(&geometry);
        // A finite-basis dianion is a numerical regression, not a bound-species claim.
        let molecule = Molecule::try_new(
            geometry,
            crate::molecules::Units::Bohr,
            -2,
            std::num::NonZeroU8::MIN,
        )
        .unwrap();
        let mut uhf = UhfCalculation::new(
            &molecule,
            &basis,
            1,
            1e-10,
            1e-8,
            crate::hf::density_guess::zero::Zero,
        )
        .unwrap();
        assert!(!uhf.density_has_required_population);
        uhf.enable_diis(6).unwrap();
        let result = uhf.run().unwrap();
        assert!(matches!(result, ScfOutcome::Unconverged(_)));
        assert!(uhf.energy > 0.2);
        assert!(uhf.density_has_required_population);
        for density in uhf.density.as_ref() {
            assert_abs_diff_eq!(
                (density * &uhf.overlap_matrix).trace(),
                2.0,
                epsilon = 1e-12
            );
        }
        assert_abs_diff_eq!(
            result.summary().spin.unwrap().s_squared,
            0.0,
            epsilon = 1e-12
        );
        test_utils::write_damping_reference(
            "h2-dianion-uhf",
            &serde_json::json!({
            "system": "h2-dianion", "method": "uhf", "nao": 2,
            "density": [uhf.density.alpha.as_slice(), uhf.density.beta.as_slice()],
            "fock": [uhf.fock.alpha.as_slice(), uhf.fock.beta.as_slice()], "energy": uhf.energy}),
        );
    }

    #[test]
    fn optimal_damping_retains_exact_endpoint_matrices() {
        let (molecule, basis) = closed_shell_h2();
        let mut uhf =
            UhfCalculation::new(&molecule, &basis, 1, 1e-8, 1e-8, CoreHamiltonian::default())
                .unwrap();
        uhf.update_fock_matrices();
        let ground_density = uhf.density.clone();
        let ground_fock = uhf.fock.clone();
        let rotation = DMatrix::from_row_slice(2, 2, &[1.0, -1.0, 1.0, 1.0]) / 2.0_f64.sqrt();
        uhf.mo_coefficients = uhf
            .mo_coefficients
            .as_ref()
            .map(|coefficients| coefficients * &rotation);
        uhf.update_density_matrices();
        uhf.update_fock_matrices();
        let trial_density = uhf.density.clone();
        let trial_fock = uhf.fock.clone();
        let zero = uhf
            .apply_optimal_damping(&ground_density, &ground_fock)
            .unwrap();
        assert_abs_diff_eq!(zero, 0.0, epsilon = 0.0);
        let one = uhf
            .apply_optimal_damping(&trial_density, &trial_fock)
            .unwrap();
        assert_abs_diff_eq!(one, 1.0, epsilon = 0.0);
        assert_eq!(uhf.density.alpha, ground_density.alpha);
        assert_eq!(uhf.density.beta, ground_density.beta);
        assert_eq!(uhf.fock.alpha, ground_fock.alpha);
        assert_eq!(uhf.fock.beta, ground_fock.beta);
    }

    fn perturbed_core_guess(seed: u64, std_dev: f64) -> CoreHamiltonian {
        CoreHamiltonian::new(Some(GuessPerturbationConfig {
            random: RandomConfig {
                distribution: DistributionConfig::Normal {
                    config: NormalDistributionConfig {
                        mean: 0.0,
                        std_dev: PositiveFiniteF64::try_new(std_dev).unwrap(),
                    },
                },
                seed: Some(seed),
            },
        }))
    }

    fn assert_symmetric(matrix: &DMatrix<f64>) {
        assert_abs_diff_eq!(matrix, &matrix.transpose(), epsilon = 1e-10);
    }

    fn assert_final_densities_match_canonical_orbitals(uhf: &UhfCalculation<'_>, epsilon: f64) {
        let alpha_density =
            density_from_mo_coefficients(&uhf.mo_coefficients.alpha, uhf.occupied_orbitals.alpha);
        let beta_density =
            density_from_mo_coefficients(&uhf.mo_coefficients.beta, uhf.occupied_orbitals.beta);

        assert!(
            (&alpha_density - &uhf.density.alpha).norm() < epsilon,
            "final canonical alpha orbitals do not reproduce the stored UHF density"
        );
        assert!(
            (&beta_density - &uhf.density.beta).norm() < epsilon,
            "final canonical beta orbitals do not reproduce the stored UHF density"
        );
    }

    fn closed_shell_h2() -> (Molecule, Basis) {
        let geometry = test_utils::load_sample_geometry_in_bohr("samples/h2/molecule.xyz");
        let basis = test_utils::load_sto3g_basis(&geometry);
        let molecule = Molecule::try_new(
            geometry,
            crate::molecules::Units::Bohr,
            0,
            std::num::NonZeroU8::MIN,
        )
        .unwrap();
        (molecule, basis)
    }

    #[test]
    fn spin_fock_matrices_match_independent_contractions() {
        let geometry = test_utils::load_sample_geometry_in_bohr("samples/oh/oh.xyz");
        let basis = test_utils::load_sto3g_basis(&geometry);
        let molecule = Molecule::try_new(
            geometry,
            crate::molecules::Units::Bohr,
            0,
            std::num::NonZeroU8::new(2).unwrap(),
        )
        .unwrap();
        let uhf = UhfCalculation::new(&molecule, &basis, 100, 1e-8, 1e-8, OneElectron::default())
            .unwrap();
        let n = basis.nbasis();
        // Unequal off-diagonal spin densities detect accidentally shared exchange terms.
        let alpha = DMatrix::from_fn(n, n, |i, j| if i == j { 0.4 } else { -0.03 });
        let beta = DMatrix::from_fn(n, n, |i, j| if i == j { 0.2 } else { 0.01 });
        for density in [
            SpinMatrices {
                alpha: alpha.clone(),
                beta,
            },
            SpinMatrices {
                alpha: alpha.clone(),
                beta: alpha.clone(),
            },
            SpinMatrices {
                alpha,
                beta: DMatrix::zeros(n, n),
            },
        ] {
            let total_density = &density.alpha + &density.beta;
            let reference = |spin: &DMatrix<f64>| {
                DMatrix::from_fn(n, n, |mu, nu| {
                    let mut correction = 0.0;
                    for lambda in 0..n {
                        for sigma in 0..n {
                            correction += total_density[(lambda, sigma)]
                                * uhf.two_electron_integrals[(mu, nu, lambda, sigma)]
                                - spin[(lambda, sigma)]
                                    * uhf.two_electron_integrals[(mu, sigma, lambda, nu)];
                        }
                    }
                    uhf.h_core[(mu, nu)] + correction
                })
            };
            let expected_alpha = reference(&density.alpha);
            let expected_beta = reference(&density.beta);
            for threads in [1, 2, 12] {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap();
                let actual = pool.install(|| uhf.build_fock_matrices(&density));
                assert_abs_diff_eq!(actual.alpha, expected_alpha, epsilon = 1e-12);
                assert_abs_diff_eq!(actual.beta, expected_beta, epsilon = 1e-12);
                assert_symmetric(&actual.alpha);
                assert_symmetric(&actual.beta);
            }
        }
    }

    #[test]
    fn test_perturbed_uhf_guess_has_distinct_normalized_spin_densities() {
        let (molecule, basis) = closed_shell_h2();
        let uhf = UhfCalculation::new(
            &molecule,
            &basis,
            100,
            1e-8,
            1e-8,
            perturbed_core_guess(42, 1e-4),
        )
        .unwrap();

        let overlap = basis.overlap_ints();
        assert!(
            (&uhf.density.alpha - &uhf.density.beta).norm() > 1e-10,
            "spin-resolved initial densities must differ"
        );
        for (density, coefficients) in [
            (&uhf.density.alpha, &uhf.mo_coefficients.alpha),
            (&uhf.density.beta, &uhf.mo_coefficients.beta),
        ] {
            assert_symmetric(density);
            assert_abs_diff_eq!((density * &overlap).trace(), 1.0, epsilon = 1e-8);
            assert_abs_diff_eq!(
                coefficients.transpose() * &overlap * coefficients,
                DMatrix::identity(coefficients.ncols(), coefficients.ncols()),
                epsilon = 1e-8
            );
        }
    }

    #[test]
    fn test_seeded_perturbed_uhf_guess_is_reproducible() {
        let (molecule, basis) = closed_shell_h2();
        let first = UhfCalculation::new(
            &molecule,
            &basis,
            100,
            1e-8,
            1e-8,
            perturbed_core_guess(7, 1e-4),
        )
        .unwrap();
        let second = UhfCalculation::new(
            &molecule,
            &basis,
            100,
            1e-8,
            1e-8,
            perturbed_core_guess(7, 1e-4),
        )
        .unwrap();

        assert_eq!(first.density.alpha, second.density.alpha);
        assert_eq!(first.density.beta, second.density.beta);
        assert_eq!(first.mo_coefficients.alpha, second.mo_coefficients.alpha);
        assert_eq!(first.mo_coefficients.beta, second.mo_coefficients.beta);
    }

    #[test]
    fn test_uhf_h2_singlet_matches_rhf_reference_energy() {
        const PYSCF_RHF_ELECTRONIC_ENERGY: f64 = -1.831_863_646_477_507;
        const PYSCF_RHF_TOTAL_ENERGY: f64 = -1.116_759_307_396_425;

        let geometry = test_utils::load_sample_geometry_in_bohr("samples/h2/molecule.xyz");
        let basis = test_utils::load_sto3g_basis(&geometry);
        let molecule = Molecule::try_new(
            geometry,
            crate::molecules::Units::Bohr,
            0,
            std::num::NonZeroU8::MIN,
        )
        .unwrap();
        let mut uhf =
            UhfCalculation::new(&molecule, &basis, 100, 1e-8, 1e-8, OneElectron::default())
                .unwrap();

        let result = uhf.run().unwrap();

        assert!(matches!(
            result,
            crate::hf::scf_result::ScfOutcome::Converged(_)
        ));
        assert_abs_diff_eq!(
            result.summary().electronic_energy,
            PYSCF_RHF_ELECTRONIC_ENERGY,
            epsilon = 1e-8
        );
        assert_abs_diff_eq!(
            result.summary().total_energy,
            PYSCF_RHF_TOTAL_ENERGY,
            epsilon = 1e-8
        );
    }

    #[rstest::rstest]
    #[case(4.0, false, -0.935_842_328_314)]
    #[case(4.0, true, -0.935_842_328_314)]
    #[case(6.0, true, -0.933_227_347_051_676)]
    fn test_stretched_h2_uhf_breaks_spin_symmetry_and_lowers_energy(
        #[case] distance: f64,
        #[case] diis_enabled: bool,
        #[case] pyscf_total_energy: f64,
    ) {
        // Independent PySCF UHF references seeded with atom-localized spins.
        // Comparing two converged solutions does not establish global stability.
        let mut geometry = test_utils::load_sample_geometry_in_bohr("samples/h2/molecule.xyz");
        geometry.atoms[0].position.z = -0.5 * distance;
        geometry.atoms[1].position.z = 0.5 * distance;
        let basis = test_utils::load_sto3g_basis(&geometry);
        let molecule = Molecule::try_new(
            geometry,
            crate::molecules::Units::Bohr,
            0,
            std::num::NonZeroU8::MIN,
        )
        .unwrap();

        let mut common = UhfCalculation::new(
            &molecule,
            &basis,
            200,
            1e-10,
            1e-8,
            CoreHamiltonian::default(),
        )
        .unwrap();
        if diis_enabled {
            common.enable_diis(6).unwrap();
        }
        let common_result = common.run().unwrap();

        let mut broken = UhfCalculation::new(
            &molecule,
            &basis,
            200,
            1e-10,
            1e-8,
            perturbed_core_guess(42, 0.1),
        )
        .unwrap();
        if diis_enabled {
            broken.enable_diis(6).unwrap();
        }
        let broken_result = broken.run().unwrap();

        assert!(matches!(
            common_result,
            crate::hf::scf_result::ScfOutcome::Converged(_)
        ));
        assert!(matches!(
            broken_result,
            crate::hf::scf_result::ScfOutcome::Converged(_)
        ));
        assert!(
            (&broken.density.alpha - &broken.density.beta).norm() > 1e-5,
            "stretched H2 must retain a broken-symmetry density"
        );
        assert_final_densities_match_canonical_orbitals(&broken, 1e-10);
        assert!(
            broken_result.summary().total_energy < common_result.summary().total_energy - 1e-6,
            "broken-symmetry UHF energy ({}) must be below common-orbital energy ({})",
            broken_result.summary().total_energy,
            common_result.summary().total_energy
        );
        assert_abs_diff_eq!(
            broken_result.summary().total_energy,
            pyscf_total_energy,
            epsilon = 1e-8
        );
        let common_spin = common_result.summary().spin.unwrap();
        let broken_spin = broken_result.summary().spin.unwrap();
        assert_abs_diff_eq!(common_spin.s_squared, 0.0, epsilon = 1e-8);
        assert_abs_diff_eq!(common_spin.spin_contamination, 0.0, epsilon = 1e-8);
        assert!(broken_spin.spin_contamination > 0.1);
    }

    #[test]
    fn test_uhf_spin_density_electron_counts_match_multiplicity() {
        let geometry = test_utils::load_sample_geometry_in_bohr("samples/h2/molecule.xyz");
        let basis = test_utils::load_sto3g_basis(&geometry);
        let molecule = Molecule::try_new(
            geometry,
            crate::molecules::Units::Bohr,
            1,
            std::num::NonZeroU8::new(2).unwrap(),
        )
        .unwrap();
        let mut uhf =
            UhfCalculation::new(&molecule, &basis, 100, 1e-8, 1e-8, OneElectron::default())
                .unwrap();

        let overlap = basis.overlap_ints();
        assert_abs_diff_eq!((&uhf.density.alpha * &overlap).trace(), 1.0, epsilon = 1e-8);
        assert_abs_diff_eq!((&uhf.density.beta * &overlap).trace(), 0.0, epsilon = 1e-8);

        let result = uhf.run().unwrap();
        let overlap = basis.overlap_ints();
        let alpha_electrons = (&uhf.density.alpha * &overlap).trace();
        let beta_electrons = (&uhf.density.beta * &overlap).trace();

        assert!(matches!(
            result,
            crate::hf::scf_result::ScfOutcome::Converged(_)
        ));
        assert_abs_diff_eq!(alpha_electrons, 1.0, epsilon = 1e-8);
        assert_abs_diff_eq!(beta_electrons, 0.0, epsilon = 1e-8);
        assert_abs_diff_eq!(
            alpha_electrons - beta_electrons,
            f64::from(molecule.unpaired_electrons()),
            epsilon = 1e-8
        );
    }

    #[test]
    fn test_uhf_h2_plus_doublet_matches_pyscf_reference_energy() {
        const PYSCF_UHF_ELECTRONIC_ENERGY: f64 = -1.253_309_787_402_661_5;
        const PYSCF_UHF_TOTAL_ENERGY: f64 = -0.538_205_448_344_553_5;

        let geometry = test_utils::load_sample_geometry_in_bohr("samples/h2/molecule.xyz");
        let basis = test_utils::load_sto3g_basis(&geometry);
        let molecule = Molecule::try_new(
            geometry,
            crate::molecules::Units::Bohr,
            1,
            std::num::NonZeroU8::new(2).unwrap(),
        )
        .unwrap();
        let mut uhf =
            UhfCalculation::new(&molecule, &basis, 100, 1e-8, 1e-8, OneElectron::default())
                .unwrap();

        let result = uhf.run().unwrap();

        assert!(matches!(
            result,
            crate::hf::scf_result::ScfOutcome::Converged(_)
        ));
        assert_abs_diff_eq!(
            result.summary().electronic_energy,
            PYSCF_UHF_ELECTRONIC_ENERGY,
            epsilon = 1e-8
        );
        assert_abs_diff_eq!(
            result.summary().total_energy,
            PYSCF_UHF_TOTAL_ENERGY,
            epsilon = 1e-8
        );
        assert_final_densities_match_canonical_orbitals(&uhf, 1e-8);
        let spin = result.summary().spin.unwrap();
        assert_abs_diff_eq!(spin.s_squared, 0.75, epsilon = 1e-10);
        assert_abs_diff_eq!(spin.spin_contamination, 0.0, epsilon = 1e-10);
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "Canonicalization or cloning must preserve the previously computed state exactly"
    )]
    fn test_converged_uhf_orbitals_are_canonical_for_final_fock() {
        let geometry = test_utils::load_sample_geometry_in_bohr("samples/oh/oh.xyz");
        let basis = test_utils::load_sto3g_basis(&geometry);
        let molecule = Molecule::try_new(
            geometry,
            crate::molecules::Units::Bohr,
            0,
            std::num::NonZeroU8::new(2).unwrap(),
        )
        .unwrap();
        let mut uhf = UhfCalculation::new(
            &molecule,
            &basis,
            100,
            1e-5,
            1e-8,
            CoreHamiltonian::default(),
        )
        .unwrap();
        uhf.enable_diis(6).unwrap();

        let mut run_observer = Vec::new();
        let result = uhf
            .run_with_iterations(|iteration| run_observer.push(iteration.clone()))
            .unwrap();

        assert!(matches!(
            result,
            crate::hf::scf_result::ScfOutcome::Converged(_)
        ));
        assert_eq!(result.summary().iterations, run_observer.len());
        let last = run_observer.last().unwrap();
        assert_eq!(last.iteration, result.summary().iterations);
        assert_eq!(last.electronic_energy, result.summary().electronic_energy);
        assert_eq!(last.delta_energy, result.summary().delta_energy);
        assert_eq!(last.residual_norm, result.summary().residual_norm);
        assert_final_densities_match_canonical_orbitals(&uhf, 1e-5);
        let residuals = uhf.fock.as_ref().zip_map(
            uhf.mo_coefficients
                .as_ref()
                .zip_map(uhf.orbital_energies.as_ref(), |coefficients, energies| {
                    (coefficients, DMatrix::from_diagonal(energies))
                }),
            |fock, (coefficients, energies)| {
                (fock * coefficients - &uhf.overlap_matrix * coefficients * energies).norm()
            },
        );
        for residual in [residuals.alpha, residuals.beta] {
            assert_abs_diff_eq!(residual, 0.0, epsilon = 1e-8);
        }

        let mut observer = Vec::new();
        let mut iterations = result.summary().iterations;
        // Force a refinement even though the preceding calculation converged.
        let delta = uhf
            .canonicalize_final_fock(
                f64::INFINITY,
                &mut iterations,
                &mut |iteration: &ScfIteration| observer.push(iteration.clone()),
            )
            .unwrap();
        assert!(!observer.is_empty());
        assert_eq!(iterations, result.summary().iterations + observer.len());
        assert!(iterations <= uhf.max_iterations);
        for (offset, step) in observer.iter().enumerate() {
            assert_eq!(step.iteration, result.summary().iterations + offset + 1);
        }
        let last = observer.last().unwrap();
        assert_eq!(last.electronic_energy, uhf.energy);
        assert_eq!(last.delta_energy, delta);
        assert_eq!(last.residual_norm, uhf.residual_norm);

        // An exhausted budget may verify orbitals, but cannot update density.
        uhf.max_iterations = iterations;
        let count = observer.len();
        assert!(matches!(
            uhf.canonicalize_final_fock(
                f64::INFINITY,
                &mut iterations,
                &mut |iteration: &ScfIteration| observer.push(iteration.clone())
            ),
            Err(NumericalError::FinalizationNotConverged { .. })
        ));
        assert_eq!(iterations, uhf.max_iterations);
        assert_eq!(observer.len(), count);
    }

    #[test]
    fn test_oh_uhf_is_invariant_to_rotation_and_tiny_guess_perturbations() {
        for direction in [
            nalgebra::Vector3::z(),
            nalgebra::Vector3::x(),
            nalgebra::Vector3::y(),
            nalgebra::Vector3::new(1.0, 2.0, 3.0).normalize(),
        ] {
            let mut geometry = test_utils::load_sample_geometry_in_bohr("samples/oh/oh.xyz");
            let distance = geometry.atoms[1].position.z;
            geometry.atoms[1].position = (direction * distance).into();
            let basis = test_utils::load_sto3g_basis(&geometry);
            let molecule = Molecule::try_new(
                geometry,
                crate::molecules::Units::Bohr,
                0,
                std::num::NonZeroU8::new(2).unwrap(),
            )
            .unwrap();
            // Tiny spin perturbations exercise different choices within the degenerate
            // pi subspace, as can occur with platform-dependent rounding.
            for seed in 0..4 {
                let guess = if seed == 0 {
                    CoreHamiltonian::default()
                } else {
                    perturbed_core_guess(seed, 1e-12)
                };
                let mut uhf =
                    UhfCalculation::new(&molecule, &basis, 100, 1e-8, 1e-8, guess).unwrap();
                uhf.enable_diis(6).unwrap();
                let result = uhf.run().unwrap();
                assert!(
                    matches!(result, ScfOutcome::Converged(_)),
                    "direction={direction:?}, seed={seed}"
                );
                assert_abs_diff_eq!(
                    result.summary().total_energy,
                    -74.362_669_194_767_24,
                    epsilon = 5e-8
                );
                assert_final_densities_match_canonical_orbitals(&uhf, 1e-8);
                let mp2 = crate::mp2::uhf_unrestricted(&uhf, 0).unwrap();
                assert_abs_diff_eq!(
                    mp2.correlation_energy,
                    -0.015_810_842_704_457_904,
                    epsilon = 5e-9
                );
            }
        }
    }

    #[test]
    fn test_uhf_oh_doublet_preserves_scf_invariants() {
        const PYSCF_UHF_ELECTRONIC_ENERGY: f64 = -78.727_017_326_066_2;
        const PYSCF_UHF_TOTAL_ENERGY: f64 = -74.362_669_194_767_24;
        // PySCF 2.14.0 with conv_tol=1e-10 on the same OH/STO-3G geometry.
        const PYSCF_S_SQUARED: f64 = 0.753_261_943_086_881_7;
        const PYSCF_SPIN_CONTAMINATION: f64 = 0.003_261_943_086_881_7;

        let geometry = test_utils::load_sample_geometry_in_bohr("samples/oh/oh.xyz");
        let basis = test_utils::load_sto3g_basis(&geometry);
        let molecule = Molecule::try_new(
            geometry,
            crate::molecules::Units::Bohr,
            0,
            std::num::NonZeroU8::new(2).unwrap(),
        )
        .unwrap();
        let mut uhf = UhfCalculation::new(
            &molecule,
            &basis,
            100,
            1e-10,
            1e-8,
            CoreHamiltonian::default(),
        )
        .unwrap();
        uhf.enable_diis(6).unwrap();

        let overlap = basis.overlap_ints();
        let initial_electrons = uhf
            .density
            .as_ref()
            .zip_map(Spin::duplicate(&overlap), |density, overlap| {
                (density * overlap).trace()
            });

        assert_abs_diff_eq!(initial_electrons.alpha, 5.0, epsilon = 1e-8);
        assert_abs_diff_eq!(initial_electrons.beta, 4.0, epsilon = 1e-8);

        let result = uhf.run().unwrap();
        let electrons = uhf
            .density
            .as_ref()
            .zip_map(Spin::duplicate(&overlap), |density, overlap| {
                (density * overlap).trace()
            });

        // OH has degenerate pi orbitals. Their orientation may vary by linear-algebra
        // backend, but the SCF energy and spin-resolved electron counts are invariant.
        assert!(matches!(
            result,
            crate::hf::scf_result::ScfOutcome::Converged(_)
        ));
        assert_abs_diff_eq!(
            result.summary().electronic_energy,
            PYSCF_UHF_ELECTRONIC_ENERGY,
            epsilon = 5e-7
        );
        assert_abs_diff_eq!(
            result.summary().total_energy,
            PYSCF_UHF_TOTAL_ENERGY,
            epsilon = 5e-7
        );
        assert_abs_diff_eq!(electrons.alpha, 5.0, epsilon = 1e-8);
        assert_abs_diff_eq!(electrons.beta, 4.0, epsilon = 1e-8);
        let spin = result.summary().spin.unwrap();
        assert_abs_diff_eq!(spin.s_squared, PYSCF_S_SQUARED, epsilon = 1e-7);
        assert_abs_diff_eq!(spin.ideal_s_squared, 0.75, epsilon = 1e-12);
        assert_abs_diff_eq!(
            spin.spin_contamination,
            PYSCF_SPIN_CONTAMINATION,
            epsilon = 1e-7
        );
    }
}
