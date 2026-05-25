use super::{density_from_fock_like_matrix, DensityGuess};
use crate::basis::gaussian::basis::Basis;
use crate::molecules::molecule::Molecule;
use nalgebra::DMatrix;
use rand::distr::{Distribution, Uniform};

pub struct RandomSymmetric;

impl DensityGuess for RandomSymmetric {
    fn build_density_guess(
        &self,
        _h_core: &DMatrix<f64>,
        molecule: &Molecule,
        basis: &Basis,
    ) -> DMatrix<f64> {
        let nbasis = basis.nbasis();
        let mut rng = rand::rng();
        let dist = Uniform::new(-1f64, 1f64).unwrap();
        let mut random_matrix = DMatrix::zeros(nbasis, nbasis);
        for i in 0..nbasis {
            for j in i..nbasis {
                let value = dist.sample(&mut rng);
                random_matrix[(i, j)] = value;
                if i != j {
                    random_matrix[(j, i)] = value;
                }
            }
        }

        density_from_fock_like_matrix(&random_matrix, molecule, basis)
    }
}
