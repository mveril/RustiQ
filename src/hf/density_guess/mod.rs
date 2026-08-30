use self::one_electron::OneElectron;
use self::random::Random;
use crate::basis::gaussian::basis::Basis;
use crate::hf::density_guess::core_hamiltonian::CoreHamiltonian;
use crate::hf::density_guess::zero::Zero;
use crate::hf::numerical_error::{ensure_finite_values, NumericalError};
use crate::hf::uhf::Spin;
use crate::runfile::hf::{DensityGuessConfig, GuessPerturbationConfig};
use crate::runfile::random_config::distribution_config::{
    DistributionCreationError, RandomSampler,
};
use nalgebra::{DMatrix, DVector};
use std::error::Error;
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum DensityGuessError {
    #[error("random distribution creation failed: {0}")]
    DistributionCreation(#[from] DistributionCreationError),
    #[error(transparent)]
    Numerical(#[from] NumericalError),
}

pub(crate) mod core_hamiltonian;
pub(crate) mod one_electron;
pub(crate) mod random;
pub(crate) mod zero;

pub(crate) trait DensityGuess: Send + Sync {
    type Error: Error;
    fn build_orbital_guess(
        &self,
        h_core: &DMatrix<f64>,
        basis: &Basis,
    ) -> Result<OrbitalGuess, Self::Error>;
}

/// Input to the shared SCF orbital-to-density construction path.
///
/// Strategies cannot return an arbitrary AO density matrix: normal guesses provide
/// symmetric Fock-like matrices, while `Zero` is an explicit startup sentinel.
pub(crate) enum OrbitalGuess {
    CommonFockLike(DMatrix<f64>),
    UnrestrictedFockLike(Spin<DMatrix<f64>>),
    Zero,
}

impl DensityGuess for DensityGuessConfig {
    type Error = DensityGuessError;

    fn build_orbital_guess(
        &self,
        h_core: &DMatrix<f64>,
        basis: &Basis,
    ) -> Result<OrbitalGuess, Self::Error> {
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
            DensityGuessConfig::Zero => Ok(Zero::build_orbital_guess(&Zero, h_core, basis)
                .unwrap_or_else(|error| match error {})),
        }
    }
}

pub(crate) fn unrestricted_perturb_fock_like_matrices(
    fock_like: &DMatrix<f64>,
    perturbation: GuessPerturbationConfig,
) -> Result<OrbitalGuess, DistributionCreationError> {
    let mut sampler = perturbation.random.sample_iter()?;
    Ok(OrbitalGuess::UnrestrictedFockLike(Spin::new(
        fock_like + symmetric_random_matrix(fock_like.nrows(), &mut sampler)?,
        fock_like + symmetric_random_matrix(fock_like.nrows(), &mut sampler)?,
    )))
}

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
    let sorted_mo_coefficients = sort_orbitals(mo_coefficients, eig.eigenvalues)?;
    Ok(sorted_mo_coefficients)
}

fn sort_orbitals(
    mo_coefficients: DMatrix<f64>,
    orbital_energies: DVector<f64>,
) -> Result<DMatrix<f64>, NumericalError> {
    ensure_finite_values(&orbital_energies, "orbital energies")?;
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
    use crate::hf::core::core_hamiltonian_ints;
    use crate::hf::orthogonalization::orthogonalizer as build_orthogonalizer;
    use crate::molecules::molecule::Molecule;
    use crate::runfile::hf::{GuessPerturbationConfig, RandomGuessConfig};
    use crate::runfile::random_config::distribution_config::NormalDistributionConfig;
    use crate::runfile::random_config::DistributionConfig;
    use crate::runfile::validated::PositiveFiniteF64;
    use crate::test_utils;
    use std::mem::discriminant;
    use toml_spanner::Toml;

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
                OrbitalGuess::CommonFockLike(fock_like) => {
                    let coefficients =
                        mo_coefficients_from_fock_like_matrix(&fock_like, orthogonalizer)?;
                    let occupied = coefficients.columns(0, molecule.occupied_orbitals());
                    Ok(2.0 * occupied * occupied.transpose())
                }
                OrbitalGuess::UnrestrictedFockLike(Spin { alpha, .. }) => {
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

    fn h2_system() -> (Molecule, Basis, DMatrix<f64>) {
        let geometry = test_utils::load_sample_geometry_in_bohr("samples/h2/molecule.xyz");
        let basis = test_utils::load_sto3g_basis(&geometry);
        let molecule = Molecule::try_new(
            geometry,
            crate::molecules::units::Units::Bohr,
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
            random: crate::runfile::random_config::RandomConfig {
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
                    "matrix is not symmetric at ({}, {})",
                    i,
                    j
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

    #[test]
    fn test_density_guess_type_deserialization() {
        #[derive(Toml)]
        #[toml(FromToml)]
        struct GuessConfig {
            guess: crate::runfile::hf::DensityGuessConfig,
        }

        for (toml, expected) in [
            (
                r#"
                [guess]
                type = "OneElectron"
                "#,
                DensityGuessConfig::OneElectron { perturbation: None },
            ),
            (
                r#"
                [guess]
                type = "Random"
                distribution = "Uniform"
                min = -1.0
                max = 1.0
                "#,
                DensityGuessConfig::Random {
                    config: RandomGuessConfig::default(),
                },
            ),
            (
                r#"
                [guess]
                type = "Zero"
                "#,
                DensityGuessConfig::Zero,
            ),
            (
                r#"
                [guess]
                type = "CoreHamiltonian"
                "#,
                DensityGuessConfig::CoreHamiltonian { perturbation: None },
            ),
        ] {
            let config: GuessConfig = toml_spanner::from_str(toml).unwrap();
            assert_eq!(discriminant(&config.guess), discriminant(&expected));
            let _density_guess = config.guess;
        }
    }
}
