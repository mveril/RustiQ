use self::one_electron::OneElectron;
use self::random::Random;
use crate::basis::gaussian::basis::Basis;
use crate::hf::density_guess::core_hamiltonian::CoreHamiltonian;
use crate::hf::density_guess::random_symmetric::RandomSymmetric;
use crate::hf::density_guess::zero::Zero;
use crate::molecules::molecule::Molecule;
use crate::runfile::hf::{DensityGuessConfig, GuessPerturbationConfig, HfConfig};
use nalgebra::{DMatrix, DVector};

pub(crate) mod core_hamiltonian;
pub(crate) mod one_electron;
pub(crate) mod random;
pub(crate) mod random_symmetric;
pub(crate) mod zero;

pub trait DensityGuess: Send + Sync {
    fn build_density_guess(
        &self,
        h_core: &DMatrix<f64>,
        molecule: &Molecule,
        basis: &Basis,
    ) -> DMatrix<f64>;
}

impl DensityGuess for Box<dyn DensityGuess> {
    fn build_density_guess(
        &self,
        h_core: &DMatrix<f64>,
        molecule: &Molecule,
        basis: &Basis,
    ) -> DMatrix<f64> {
        self.as_ref().build_density_guess(h_core, molecule, basis)
    }
}

#[cfg(test)]
impl DensityGuessConfig {
    pub fn get_density_guess(&self) -> Box<dyn DensityGuess> {
        match *self {
            Self::OneElectron { perturbation } => Box::new(OneElectron::new(perturbation)),
            Self::Random { config } => Box::new(Random::new(config)),
            Self::Zero => Box::new(Zero),
            Self::CoreHamiltonian { perturbation } => Box::new(CoreHamiltonian::new(perturbation)),
            Self::RandomSymmetric { config } => Box::new(RandomSymmetric::new(config)),
        }
    }
}

impl HfConfig {
    pub(crate) fn get_density_guess(&self) -> Box<dyn DensityGuess> {
        match self.guess {
            DensityGuessConfig::OneElectron { perturbation } => {
                Box::new(OneElectron::new(perturbation))
            }
            DensityGuessConfig::Random { config } => Box::new(Random::new(config)),
            DensityGuessConfig::Zero => Box::new(Zero),
            DensityGuessConfig::CoreHamiltonian { perturbation } => {
                Box::new(CoreHamiltonian::new(perturbation))
            }
            DensityGuessConfig::RandomSymmetric { config } => {
                Box::new(RandomSymmetric::new(config))
            }
        }
    }
}

pub(crate) fn perturb_fock_like_matrix(
    fock_like: &DMatrix<f64>,
    perturbation: Option<GuessPerturbationConfig>,
) -> DMatrix<f64> {
    let Some(perturbation) = perturbation else {
        return fock_like.clone();
    };
    fock_like + symmetric_random_matrix(fock_like.nrows(), perturbation)
}

fn symmetric_random_matrix(size: usize, perturbation: GuessPerturbationConfig) -> DMatrix<f64> {
    let mut rng = perturbation.random.sample_iter();
    let mut matrix = DMatrix::zeros(size, size);
    for i in 0..size {
        for j in i..size {
            let value = rng.next().unwrap();
            matrix[(i, j)] = value;
            if i != j {
                matrix[(j, i)] = value;
            }
        }
    }
    matrix
}

pub(crate) fn density_from_fock_like_matrix(
    fock_like: &DMatrix<f64>,
    molecule: &Molecule,
    basis: &Basis,
) -> DMatrix<f64> {
    let overlap = basis.overlap_ints();
    let s_inv_sqrt = symmetric_orthogonalizer(&overlap);
    let orthogonal_fock = &s_inv_sqrt.transpose() * fock_like * &s_inv_sqrt;
    let eig = orthogonal_fock.symmetric_eigen();
    let mo_coefficients = &s_inv_sqrt * eig.eigenvectors;
    let sorted_mo_coefficients = sort_orbitals(mo_coefficients, eig.eigenvalues);
    density_from_mo_coefficients(&sorted_mo_coefficients, molecule.occupied_orbitals())
}

pub(crate) fn density_from_mo_coefficients(
    mo_coefficients: &DMatrix<f64>,
    occupied_orbitals: usize,
) -> DMatrix<f64> {
    let occupied_columns: Vec<usize> = (0..occupied_orbitals).collect();
    let c_occ = mo_coefficients.select_columns(&occupied_columns);
    2.0 * &c_occ * &c_occ.transpose()
}

fn sort_orbitals(mo_coefficients: DMatrix<f64>, orbital_energies: DVector<f64>) -> DMatrix<f64> {
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
    DMatrix::from_columns(&sorted_vectors)
}

fn symmetric_orthogonalizer(overlap: &DMatrix<f64>) -> DMatrix<f64> {
    let eig = overlap.clone().symmetric_eigen();
    let inv_sqrt_values = eig.eigenvalues.map(|value| {
        assert!(value > 0.0, "Overlap matrix S is not positive definite.");
        1.0 / value.sqrt()
    });
    &eig.eigenvectors * DMatrix::from_diagonal(&inv_sqrt_values) * eig.eigenvectors.transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hf::core::core_hamiltonian_ints;
    use crate::runfile::hf::{GuessPerturbationConfig, RandomGuessConfig};
    use crate::runfile::random_config::distribution_config::NormalDistributionConfig;
    use crate::runfile::random_config::{DistributionConfig, RandomConfig};
    use crate::test_utils;
    use serde::Deserialize;
    use std::mem::discriminant;

    fn h2_system() -> (Molecule, Basis, DMatrix<f64>) {
        let geometry = test_utils::load_sample_geometry_in_bohr("samples/h2/molecule.xyz");
        let basis = test_utils::load_sto3g_basis(&geometry);
        let molecule = Molecule::from(geometry);
        let (t_matrix, v_matrix) = core_hamiltonian_ints(&molecule, &basis);
        let h_core = t_matrix + v_matrix;
        (molecule, basis, h_core)
    }

    fn perturbation(seed: u64) -> GuessPerturbationConfig {
        GuessPerturbationConfig {
            random: RandomConfig {
                distribution: DistributionConfig::Normal(NormalDistributionConfig {
                    mean: 0.0,
                    std_dev: 1e-4,
                }),
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

    #[test]
    fn test_all_density_guesses_have_expected_shape_and_finite_values() {
        let (molecule, basis, h_core) = h2_system();

        for guess_type in [
            DensityGuessConfig::CoreHamiltonian { perturbation: None },
            DensityGuessConfig::OneElectron { perturbation: None },
            DensityGuessConfig::Random {
                config: RandomGuessConfig::default(),
            },
            DensityGuessConfig::RandomSymmetric {
                config: RandomGuessConfig::default(),
            },
            DensityGuessConfig::Zero,
        ] {
            let density = guess_type
                .get_density_guess()
                .build_density_guess(&h_core, &molecule, &basis);

            assert_density_shape(&density, &basis);
            assert_finite(&density);
        }
    }

    #[test]
    fn test_zero_density_guess() {
        let (molecule, basis, h_core) = h2_system();
        let density = Zero.build_density_guess(&h_core, &molecule, &basis);

        assert_density_shape(&density, &basis);
        assert_symmetric(&density);
        assert_eq!(density, DMatrix::zeros(basis.nbasis(), basis.nbasis()));
    }

    #[test]
    fn test_random_density_guess_has_expected_range() {
        let (molecule, basis, h_core) = h2_system();
        let density = Random::default().build_density_guess(&h_core, &molecule, &basis);

        assert_density_shape(&density, &basis);
        assert_finite(&density);
        for value in density.iter() {
            assert!(
                (-1.0..=1.0).contains(value),
                "random density value {value} is outside [-1, 1]"
            );
        }
    }

    #[test]
    fn test_symmetric_density_guesses_are_symmetric() {
        let (molecule, basis, h_core) = h2_system();

        for guess in [
            DensityGuessConfig::CoreHamiltonian { perturbation: None }.get_density_guess(),
            DensityGuessConfig::OneElectron { perturbation: None }.get_density_guess(),
            DensityGuessConfig::RandomSymmetric {
                config: RandomGuessConfig::default(),
            }
            .get_density_guess(),
            DensityGuessConfig::Zero.get_density_guess(),
        ] {
            let density = guess.build_density_guess(&h_core, &molecule, &basis);

            assert_density_shape(&density, &basis);
            assert_symmetric(&density);
        }
    }

    #[test]
    fn test_fock_like_density_guesses_have_electron_count() {
        let (molecule, basis, h_core) = h2_system();

        for guess in [
            DensityGuessConfig::CoreHamiltonian { perturbation: None }.get_density_guess(),
            DensityGuessConfig::RandomSymmetric {
                config: RandomGuessConfig::default(),
            }
            .get_density_guess(),
        ] {
            let density = guess.build_density_guess(&h_core, &molecule, &basis);

            assert_density_shape(&density, &basis);
            assert_electron_count(&density, &molecule, &basis);
        }
    }

    #[test]
    fn test_perturbed_core_hamiltonian_guess_is_reproducible_with_seed() {
        let (molecule, basis, h_core) = h2_system();
        let first = DensityGuessConfig::CoreHamiltonian {
            perturbation: Some(perturbation(42)),
        }
        .get_density_guess()
        .build_density_guess(&h_core, &molecule, &basis);
        let second = DensityGuessConfig::CoreHamiltonian {
            perturbation: Some(perturbation(42)),
        }
        .get_density_guess()
        .build_density_guess(&h_core, &molecule, &basis);

        assert_eq!(first, second);
        assert_symmetric(&first);
        assert_finite(&first);
        assert_electron_count(&first, &molecule, &basis);
    }

    #[test]
    fn test_perturbed_core_hamiltonian_guess_changes_with_seed() {
        let (molecule, basis, h_core) = h2_system();
        let first = DensityGuessConfig::CoreHamiltonian {
            perturbation: Some(perturbation(42)),
        }
        .get_density_guess()
        .build_density_guess(&h_core, &molecule, &basis);
        let second = DensityGuessConfig::CoreHamiltonian {
            perturbation: Some(perturbation(43)),
        }
        .get_density_guess()
        .build_density_guess(&h_core, &molecule, &basis);

        assert_ne!(first, second);
    }

    #[test]
    fn test_perturbed_one_electron_guess_is_symmetric_and_reproducible() {
        let (molecule, basis, h_core) = h2_system();
        let first = DensityGuessConfig::OneElectron {
            perturbation: Some(perturbation(42)),
        }
        .get_density_guess()
        .build_density_guess(&h_core, &molecule, &basis);
        let second = DensityGuessConfig::OneElectron {
            perturbation: Some(perturbation(42)),
        }
        .get_density_guess()
        .build_density_guess(&h_core, &molecule, &basis);

        assert_eq!(first, second);
        assert_symmetric(&first);
        assert_finite(&first);
    }

    #[test]
    fn test_density_guess_type_deserialization() {
        #[derive(Deserialize)]
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
            (
                r#"
                [guess]
                type = "RandomSymmetric"
                distribution = "Uniform"
                min = -1.0
                max = 1.0
                "#,
                DensityGuessConfig::RandomSymmetric {
                    config: RandomGuessConfig::default(),
                },
            ),
        ] {
            let config: GuessConfig = toml::from_str(toml).unwrap();
            assert_eq!(discriminant(&config.guess), discriminant(&expected));
            let _density_guess = config.guess.get_density_guess();
        }
    }
}
