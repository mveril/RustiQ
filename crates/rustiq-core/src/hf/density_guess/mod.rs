use self::one_electron::OneElectron;
use self::random::Random;
use crate::basis::gaussian::basis::Basis;
use crate::config::hf::{DensityGuessConfig, GuessPerturbationConfig};
use crate::config::random_config::distribution_config::{DistributionCreationError, RandomSampler};
use crate::hf::density_guess::core_hamiltonian::CoreHamiltonian;
use crate::hf::density_guess::zero::Zero;
use crate::hf::numerical_error::{ensure_finite_values, NumericalError};
use crate::hf::reference::{HfReference, Restricted};
use nalgebra::{DMatrix, DVector};
use std::error::Error;
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DensityGuessError {
    #[error("random distribution creation failed: {0}")]
    DistributionCreation(#[from] DistributionCreationError),
    #[error(transparent)]
    Numerical(#[from] NumericalError),
}

pub(crate) mod core_hamiltonian;
pub(crate) mod one_electron;
pub(crate) mod random;
pub(crate) mod zero;

pub trait DensityGuess<R: HfReference = Restricted>: Send + Sync {
    type Error: Error;
    fn build_orbital_guess(
        &self,
        h_core: &DMatrix<f64>,
        basis: &Basis,
    ) -> Result<OrbitalGuess<R>, Self::Error>;
}

/// Input to the shared SCF orbital-to-density construction path.
///
/// Strategies cannot return an arbitrary AO density matrix: normal guesses provide
/// symmetric Fock-like matrices, while `Zero` is an explicit startup sentinel.
pub enum OrbitalGuess<R: HfReference = Restricted> {
    FockLike(R::Component<DMatrix<f64>>),
    Zero,
}

impl<R: HfReference> DensityGuess<R> for DensityGuessConfig {
    type Error = DensityGuessError;

    fn build_orbital_guess(
        &self,
        h_core: &DMatrix<f64>,
        basis: &Basis,
    ) -> Result<OrbitalGuess<R>, Self::Error> {
        match self {
            DensityGuessConfig::CoreHamiltonian { perturbation } => {
                CoreHamiltonian::new(*perturbation).build_orbital_guess(h_core, basis)
            }
            DensityGuessConfig::OneElectron { perturbation } => {
                OneElectron::new(*perturbation).build_orbital_guess(h_core, basis)
            }
            DensityGuessConfig::Random { config } => {
                Random::new(*config).build_orbital_guess(h_core, basis)
            }
            DensityGuessConfig::Zero => Ok(<Zero as DensityGuess<R>>::build_orbital_guess(
                &Zero, h_core, basis,
            )
            .unwrap_or_else(|error| match error {})),
        }
    }
}

pub(crate) fn perturb_fock_like_matrices<R: HfReference>(
    fock_like: &DMatrix<f64>,
    perturbation: GuessPerturbationConfig,
) -> Result<OrbitalGuess<R>, DistributionCreationError> {
    let mut sampler = perturbation.random.sample_iter()?;
    Ok(OrbitalGuess::FockLike(R::try_generate_component(|| {
        symmetric_random_matrix(fock_like.nrows(), &mut sampler).map(|noise| fock_like + noise)
    })?))
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "Use the same fallible interface as the perturbation callers and density-guess strategies"
)]
pub(crate) fn symmetric_random_matrix<T: RandomSampler + ?Sized>(
    size: usize,
    sampler: &mut T,
) -> Result<DMatrix<f64>, DistributionCreationError> {
    let mut matrix = DMatrix::zeros(size, size);
    for i in 0..size {
        for j in i..size {
            let value = sampler.sample();
            matrix[(i, j)] = value;
            if i != j {
                matrix[(j, i)] = value;
            }
        }
    }
    Ok(matrix)
}

pub(crate) fn mo_coefficients_from_fock_like_matrix(
    fock_like: &DMatrix<f64>,
    orthogonalizer: &DMatrix<f64>,
) -> Result<DMatrix<f64>, NumericalError> {
    let orthogonal_fock = &orthogonalizer.transpose() * fock_like * orthogonalizer;
    let eig = orthogonal_fock.symmetric_eigen();
    let mo_coefficients = orthogonalizer * eig.eigenvectors;
    let sorted_mo_coefficients = sort_orbitals(&mo_coefficients, &eig.eigenvalues)?;
    Ok(sorted_mo_coefficients)
}

fn sort_orbitals(
    mo_coefficients: &DMatrix<f64>,
    orbital_energies: &DVector<f64>,
) -> Result<DMatrix<f64>, NumericalError> {
    ensure_finite_values(orbital_energies, "orbital energies")?;
    let mut order: Vec<usize> = (0..orbital_energies.len()).collect();
    order.sort_by(|&a, &b| orbital_energies[a].total_cmp(&orbital_energies[b]));

    let sorted_vectors = order
        .iter()
        .map(|&i| mo_coefficients.column(i).into_owned())
        .collect::<Vec<_>>();
    Ok(DMatrix::from_columns(&sorted_vectors))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::hf::{GuessPerturbationConfig, RandomGuessConfig};
    use crate::config::random_config::distribution_config::NormalDistributionConfig;
    use crate::config::random_config::DistributionConfig;
    use crate::config::validated::PositiveFiniteF64;
    use crate::hf::core::core_hamiltonian_ints;
    use crate::hf::orthogonalization::orthogonalizer as build_orthogonalizer;
    use crate::hf::reference::Unrestricted;
    use crate::molecules::Molecule;
    use crate::test_utils;

    trait DensityGuessTestExt: DensityGuess
    where
        Self::Error: 'static,
    {
        fn build_density_guess(
            &self,
            h_core: &DMatrix<f64>,
            molecule: &Molecule,
            basis: &Basis,
            orthogonalizer: &DMatrix<f64>,
        ) -> Result<DMatrix<f64>, Box<dyn Error>> {
            match self.build_orbital_guess(h_core, basis)? {
                OrbitalGuess::FockLike(alpha) => {
                    let coefficients =
                        mo_coefficients_from_fock_like_matrix(&alpha, orthogonalizer)?;
                    let occupied = coefficients.columns(0, molecule.occupied_orbitals());
                    Ok(2.0 * occupied * occupied.transpose())
                }
                OrbitalGuess::Zero => Ok(DMatrix::zeros(basis.nbasis(), basis.nbasis())),
            }
        }
    }

    impl<T> DensityGuessTestExt for T
    where
        T: DensityGuess,
        T::Error: 'static,
    {
    }

    #[test]
    fn unperturbed_guesses_share_alpha_and_beta_fock_matrices() {
        let (_, basis, h_core) = h2_system();
        for config in [
            DensityGuessConfig::CoreHamiltonian { perturbation: None },
            DensityGuessConfig::OneElectron { perturbation: None },
        ] {
            let restricted: OrbitalGuess<Restricted> =
                config.build_orbital_guess(&h_core, &basis).unwrap();
            let unrestricted: OrbitalGuess<Unrestricted> =
                config.build_orbital_guess(&h_core, &basis).unwrap();
            let (OrbitalGuess::FockLike(shared), OrbitalGuess::FockLike(spin)) =
                (restricted, unrestricted)
            else {
                panic!("unperturbed guesses must contain Fock-like matrices");
            };
            assert_eq!(shared, h_core);
            assert_eq!(spin.alpha(), &shared);
            assert_eq!(spin.beta(), &shared);
        }
    }

    #[test]
    fn seeded_guesses_preserve_reference_layout_and_alpha_sequence() {
        let (_, basis, h_core) = h2_system();
        for config in [
            DensityGuessConfig::Random {
                config: RandomGuessConfig::default(),
            },
            DensityGuessConfig::CoreHamiltonian {
                perturbation: Some(perturbation(42)),
            },
            DensityGuessConfig::OneElectron {
                perturbation: Some(perturbation(42)),
            },
        ] {
            // Explicit seeds make both references consume the same spin sequence.
            let config = match config {
                DensityGuessConfig::Random { mut config } => {
                    config.random.seed = Some(42);
                    DensityGuessConfig::Random { config }
                }
                config => config,
            };
            let restricted: OrbitalGuess<Restricted> =
                config.build_orbital_guess(&h_core, &basis).unwrap();
            let unrestricted: OrbitalGuess<Unrestricted> =
                config.build_orbital_guess(&h_core, &basis).unwrap();
            let (OrbitalGuess::FockLike(shared), OrbitalGuess::FockLike(spin)) =
                (restricted, unrestricted)
            else {
                panic!("random and perturbed guesses must use reference-specific matrices");
            };
            assert_eq!(shared, spin.alpha);
            assert_ne!(spin.alpha, spin.beta);
            let overlap = basis.overlap_ints();
            let orthogonalizer = orthogonalizer(&basis);
            for fock_like in [&shared, &spin.alpha, &spin.beta] {
                let coefficients =
                    mo_coefficients_from_fock_like_matrix(fock_like, &orthogonalizer).unwrap();
                let metric = coefficients.transpose() * &overlap * &coefficients;
                assert!(
                    (metric - DMatrix::identity(coefficients.ncols(), coefficients.ncols())).norm()
                        < 1e-10
                );
                let occupied = coefficients.columns(0, 1);
                let density = occupied * occupied.transpose();
                assert!(((&density * &overlap).trace() - 1.0).abs() < 1e-10);
                assert!((&density * &overlap * &density - &density).norm() < 1e-10);
            }
        }
    }

    fn h2_system() -> (Molecule, Basis, DMatrix<f64>) {
        let geometry = test_utils::load_sample_geometry_in_bohr("samples/h2/molecule.xyz");
        let basis = test_utils::load_sto3g_basis(&geometry);
        let molecule = Molecule::try_new(
            geometry,
            crate::molecules::Units::Bohr,
            0,
            std::num::NonZeroU8::MIN,
        )
        .unwrap();
        let (t_matrix, v_matrix) = core_hamiltonian_ints(&molecule, &basis);
        let h_core = t_matrix + v_matrix;
        (molecule, basis, h_core)
    }

    fn perturbation(seed: u64) -> GuessPerturbationConfig {
        GuessPerturbationConfig {
            random: crate::config::random_config::RandomConfig {
                distribution: DistributionConfig::Normal {
                    config: NormalDistributionConfig {
                        mean: 0.0,
                        std_dev: PositiveFiniteF64::try_new(1e-4).unwrap(),
                    },
                },
                seed: Some(seed),
            },
        }
    }

    fn assert_symmetric(matrix: &DMatrix<f64>) {
        for i in 0..matrix.nrows() {
            for j in 0..matrix.ncols() {
                assert!(
                    (matrix[(i, j)] - matrix[(j, i)]).abs() < 1e-10,
                    "matrix is not symmetric at ({i}, {j})"
                );
            }
        }
    }

    fn assert_density_shape(density: &DMatrix<f64>, basis: &Basis) {
        assert_eq!(density.nrows(), basis.nbasis());
        assert_eq!(density.ncols(), basis.nbasis());
    }

    fn assert_finite(matrix: &DMatrix<f64>) {
        for value in matrix.iter() {
            assert!(value.is_finite(), "matrix contains a non-finite value");
        }
    }

    #[allow(
        clippy::cast_precision_loss,
        reason = "Fixture dimensions and quadrature orders are small enough to be represented exactly in f64"
    )]
    fn assert_electron_count(density: &DMatrix<f64>, molecule: &Molecule, basis: &Basis) {
        let electron_count = (density * basis.overlap_ints()).trace();
        assert!(
            (electron_count - molecule.total_electrons() as f64).abs() < 1e-8,
            "density electron count is {}, expected {}",
            electron_count,
            molecule.total_electrons()
        );
    }

    fn orthogonalizer(basis: &Basis) -> DMatrix<f64> {
        build_orthogonalizer(&basis.overlap_ints(), "overlap", 1e-8)
            .unwrap()
            .matrix
    }

    #[test]
    fn test_all_density_guesses_have_expected_shape_and_finite_values() {
        let (molecule, basis, h_core) = h2_system();
        let orthogonalizer = orthogonalizer(&basis);

        for guess_type in [
            DensityGuessConfig::CoreHamiltonian { perturbation: None },
            DensityGuessConfig::OneElectron { perturbation: None },
            DensityGuessConfig::Random {
                config: RandomGuessConfig::default(),
            },
            DensityGuessConfig::Zero,
        ] {
            let density = guess_type
                .build_density_guess(&h_core, &molecule, &basis, &orthogonalizer)
                .unwrap();

            assert_density_shape(&density, &basis);
            assert_finite(&density);
        }
    }

    #[test]
    fn test_zero_density_guess() {
        let (molecule, basis, h_core) = h2_system();
        let orthogonalizer = orthogonalizer(&basis);
        let density = Zero
            .build_density_guess(&h_core, &molecule, &basis, &orthogonalizer)
            .unwrap();

        assert_density_shape(&density, &basis);
        assert_symmetric(&density);
        assert_eq!(density, DMatrix::zeros(basis.nbasis(), basis.nbasis()));
    }

    #[test]
    fn test_random_density_guess_is_a_valid_symmetric_density() {
        let (molecule, basis, h_core) = h2_system();
        let orthogonalizer = orthogonalizer(&basis);
        let density = Random::default()
            .build_density_guess(&h_core, &molecule, &basis, &orthogonalizer)
            .unwrap();

        assert_density_shape(&density, &basis);
        assert_finite(&density);
        assert_symmetric(&density);
        assert_electron_count(&density, &molecule, &basis);
    }

    #[test]
    fn test_random_density_guess_is_symmetric() {
        let (molecule, basis, h_core) = h2_system();
        let orthogonalizer = orthogonalizer(&basis);

        for guess in [
            DensityGuessConfig::CoreHamiltonian { perturbation: None },
            DensityGuessConfig::OneElectron { perturbation: None },
            DensityGuessConfig::Random {
                config: RandomGuessConfig::default(),
            },
            DensityGuessConfig::Zero,
        ] {
            let density = guess
                .build_density_guess(&h_core, &molecule, &basis, &orthogonalizer)
                .unwrap();

            assert_density_shape(&density, &basis);
            assert_symmetric(&density);
        }
    }

    #[test]
    fn test_fock_like_density_guesses_have_electron_count() {
        let (molecule, basis, h_core) = h2_system();
        let orthogonalizer = orthogonalizer(&basis);

        for guess in [
            DensityGuessConfig::CoreHamiltonian { perturbation: None },
            DensityGuessConfig::OneElectron { perturbation: None },
            DensityGuessConfig::Random {
                config: RandomGuessConfig::default(),
            },
        ] {
            let density = guess
                .build_density_guess(&h_core, &molecule, &basis, &orthogonalizer)
                .unwrap();

            assert_density_shape(&density, &basis);
            assert_electron_count(&density, &molecule, &basis);
        }
    }

    #[test]
    fn test_perturbed_core_hamiltonian_guess_is_reproducible_with_seed() {
        let (molecule, basis, h_core) = h2_system();
        let orthogonalizer = orthogonalizer(&basis);
        let first = DensityGuessConfig::CoreHamiltonian {
            perturbation: Some(perturbation(42)),
        }
        .build_density_guess(&h_core, &molecule, &basis, &orthogonalizer)
        .unwrap();
        let second = DensityGuessConfig::CoreHamiltonian {
            perturbation: Some(perturbation(42)),
        }
        .build_density_guess(&h_core, &molecule, &basis, &orthogonalizer)
        .unwrap();

        assert_eq!(first, second);
        assert_symmetric(&first);
        assert_finite(&first);
        assert_electron_count(&first, &molecule, &basis);
    }

    #[test]
    fn test_perturbed_core_hamiltonian_guess_changes_with_seed() {
        let (molecule, basis, h_core) = h2_system();
        let orthogonalizer = orthogonalizer(&basis);
        let first = DensityGuessConfig::CoreHamiltonian {
            perturbation: Some(perturbation(42)),
        }
        .build_density_guess(&h_core, &molecule, &basis, &orthogonalizer)
        .unwrap();
        let second = DensityGuessConfig::CoreHamiltonian {
            perturbation: Some(perturbation(43)),
        }
        .build_density_guess(&h_core, &molecule, &basis, &orthogonalizer)
        .unwrap();

        assert_ne!(first, second);
    }

    #[test]
    fn test_perturbed_one_electron_guess_is_symmetric_and_reproducible() {
        let (molecule, basis, h_core) = h2_system();
        let orthogonalizer = orthogonalizer(&basis);
        let first = DensityGuessConfig::OneElectron {
            perturbation: Some(perturbation(42)),
        }
        .build_density_guess(&h_core, &molecule, &basis, &orthogonalizer)
        .unwrap();
        let second = DensityGuessConfig::OneElectron {
            perturbation: Some(perturbation(42)),
        }
        .build_density_guess(&h_core, &molecule, &basis, &orthogonalizer)
        .unwrap();

        assert_eq!(first, second);
        assert_symmetric(&first);
        assert_finite(&first);
    }
}
