use crate::math_utils::boys_function;
use nalgebra::{distance, distance_squared, DMatrix};
use std::f64::consts::PI;

use crate::{
    basis::gaussian::basis::{gaussian_norm_const, Basis},
    molecules::geometry::Geometry,
};

pub fn nucl_attraction_ints(mol: &Geometry, basis: &Basis) -> DMatrix<f64> {
    let n = basis.nbasis();
    let mut result = DMatrix::<f64>::zeros(n, n);

    for i in 0..n {
        let shell_i = &basis.shells[basis.shell_ids[i]];
        for j in 0..=i {
            let shell_j = &basis.shells[basis.shell_ids[j]];
            let mut integral = 0.0;

            for (&exp_i, &coeff_i) in shell_i.alpha.iter().zip(shell_i.contr[0].coeff.iter()) {
                for (&exp_j, &coeff_j) in shell_j.alpha.iter().zip(shell_j.contr[0].coeff.iter()) {
                    let alpha = exp_i + exp_j;
                    let l_i = basis.angular_momenta[i];
                    let l_j = basis.angular_momenta[j];
                    let norm_i =
                        gaussian_norm_const(exp_i, l_i.x as u32, l_i.y as u32, l_i.z as u32);
                    let norm_j =
                        gaussian_norm_const(exp_j, l_j.x as u32, l_j.y as u32, l_j.z as u32);
                    let prefactor = coeff_i
                        * coeff_j
                        * norm_i
                        * norm_j
                        * (-exp_i * exp_j * distance_squared(&shell_i.origin, &shell_j.origin)
                            / alpha)
                            .exp();
                    let p = (exp_i * shell_i.origin + exp_j * shell_j.origin.coords) / alpha;

                    for atom in &mol.atoms {
                        let z = atom.element.atomic_number as f64;
                        let r_pa = distance(&p, &atom.position);
                        let boys_arg = alpha * r_pa.powi(2);
                        let f0 = boys_function(0, boys_arg);
                        integral -= z * prefactor * f0 * (2.0 * PI / alpha);
                    }
                }
            }
            result[(i, j)] = integral;
            result[(j, i)] = integral; // Symmetry
        }
    }
    result
}

pub fn kinetic_ints(_mol: &Geometry, basis: &Basis) -> DMatrix<f64> {
    basis.kinetic_ints()
}

pub fn core_hamiltonian_ints(mol: &Geometry, basis: &Basis) -> (DMatrix<f64>, DMatrix<f64>) {
    return (kinetic_ints(mol, basis), nucl_attraction_ints(mol, basis));
}
