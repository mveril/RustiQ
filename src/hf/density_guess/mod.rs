use self::one_electron::OneElectron;
use self::random::Random;
use crate::basis::gaussian::basis::Basis;
use crate::hf::density_guess::core_hamiltonian::CoreHamiltonian;
use crate::hf::density_guess::random_symmetric::RandomSymmetric;
use crate::hf::density_guess::zero::Zero;
use crate::molecules::molecule::Molecule;
use nalgebra::{DMatrix, DVector};
use serde::{Deserialize, Serialize};

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

#[derive(Hash, Debug, Default, Serialize, Deserialize)]
pub enum GuessType {
    #[default]
    CoreHamiltonian,
    OneElectron,
    Random,
    RandomSymmetric,
    Zero,
}

impl GuessType {
    pub fn get_density_guess(&self) -> Box<dyn DensityGuess> {
        match self {
            Self::OneElectron => Box::new(OneElectron),
            Self::Random => Box::new(Random),
            Self::Zero => Box::new(Zero),
            Self::CoreHamiltonian => Box::new(CoreHamiltonian),
            Self::RandomSymmetric => Box::new(RandomSymmetric),
        }
    }
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
    use crate::test_utils;

    fn h2_system() -> (Molecule, Basis, DMatrix<f64>) {
        let geometry = test_utils::load_sample_geometry("samples/h2/molecule.xyz");
        let basis = test_utils::load_sto3g_basis(&geometry);
        let molecule = Molecule::from(geometry);
        let (t_matrix, v_matrix) = core_hamiltonian_ints(&molecule, &basis);
        let h_core = t_matrix + v_matrix;
        (molecule, basis, h_core)
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

    #[test]
    fn test_zero_density_guess() {
        let (molecule, basis, h_core) = h2_system();
        let density = Zero.build_density_guess(&h_core, &molecule, &basis);

        assert_density_shape(&density, &basis);
        assert_symmetric(&density);
        assert_eq!(density, DMatrix::zeros(basis.nbasis(), basis.nbasis()));
    }

    #[test]
    fn test_core_hamiltonian_density_guess_has_electron_count() {
        let (molecule, basis, h_core) = h2_system();
        let density = CoreHamiltonian.build_density_guess(&h_core, &molecule, &basis);

        assert_density_shape(&density, &basis);
        assert_symmetric(&density);
        let electron_count = (&density * basis.overlap_ints()).trace();
        assert!(
            (electron_count - molecule.total_electrons() as f64).abs() < 1e-8,
            "density electron count is {}, expected {}",
            electron_count,
            molecule.total_electrons()
        );
    }

    #[test]
    fn test_random_symmetric_density_guess_has_electron_count() {
        let (molecule, basis, h_core) = h2_system();
        let density = RandomSymmetric.build_density_guess(&h_core, &molecule, &basis);

        assert_density_shape(&density, &basis);
        assert_symmetric(&density);
        let electron_count = (&density * basis.overlap_ints()).trace();
        assert!(
            (electron_count - molecule.total_electrons() as f64).abs() < 1e-8,
            "density electron count is {}, expected {}",
            electron_count,
            molecule.total_electrons()
        );
    }

    #[test]
    fn test_density_guess_type_deserialization() {
        #[derive(Deserialize)]
        struct GuessConfig {
            density_guess: GuessType,
        }

        for name in [
            "OneElectron",
            "Random",
            "Zero",
            "CoreHamiltonian",
            "RandomSymmetric",
        ] {
            let config: GuessConfig =
                toml::from_str(&format!("density_guess = \"{}\"", name)).unwrap();
            let _density_guess = config.density_guess.get_density_guess();
        }
    }
}
