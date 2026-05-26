use nalgebra::DMatrix;
use rayon::prelude::*;
use std::f64::consts::PI;

use crate::{
    basis::gaussian::basis::{coulomb_auxiliary, gaussian_product_center, hermite_coeff, Basis},
    molecules::geometry::Geometry,
};

pub fn nucl_attraction_ints(mol: &Geometry, basis: &Basis) -> DMatrix<f64> {
    let n = basis.nbasis();
    let mut result = DMatrix::<f64>::zeros(n, n);

    let values = (0..n * n)
        .into_par_iter()
        .filter_map(|index| {
            let i = index / n;
            let j = index % n;
            (j <= i).then_some((i, j))
        })
        .map(|(i, j)| {
            let shell_i = &basis.shells[basis.shell_ids[i]];
            let shell_j = &basis.shells[basis.shell_ids[j]];
            let mut integral = 0.0;

            for component_i in &basis.normalized_components[i] {
                let l_i = &component_i.angular_momentum;
                for component_j in &basis.normalized_components[j] {
                    let l_j = &component_j.angular_momentum;
                    for primitive_i in &component_i.primitives {
                        let exp_i = primitive_i.exponent;
                        for primitive_j in &component_j.primitives {
                            let exp_j = primitive_j.exponent;
                            let alpha = exp_i + exp_j;
                            let p_center = gaussian_product_center(
                                exp_i,
                                &shell_i.origin,
                                exp_j,
                                &shell_j.origin,
                            );
                            let prefactor = primitive_i.coefficient * primitive_j.coefficient;

                            let e_x = (0..=l_i.x + l_j.x)
                                .map(|t| {
                                    hermite_coeff(
                                        l_i.x,
                                        l_j.x,
                                        t,
                                        shell_i.origin.x - shell_j.origin.x,
                                        exp_i,
                                        exp_j,
                                    )
                                })
                                .collect::<Vec<_>>();
                            let e_y = (0..=l_i.y + l_j.y)
                                .map(|u| {
                                    hermite_coeff(
                                        l_i.y,
                                        l_j.y,
                                        u,
                                        shell_i.origin.y - shell_j.origin.y,
                                        exp_i,
                                        exp_j,
                                    )
                                })
                                .collect::<Vec<_>>();
                            let e_z = (0..=l_i.z + l_j.z)
                                .map(|v| {
                                    hermite_coeff(
                                        l_i.z,
                                        l_j.z,
                                        v,
                                        shell_i.origin.z - shell_j.origin.z,
                                        exp_i,
                                        exp_j,
                                    )
                                })
                                .collect::<Vec<_>>();

                            for atom in &mol.atoms {
                                let z = atom.element.atomic_number as f64;
                                let pc = p_center - atom.position.coords;
                                let mut primitive = 0.0;
                                for t in 0..=l_i.x + l_j.x {
                                    for u in 0..=l_i.y + l_j.y {
                                        for v in 0..=l_i.z + l_j.z {
                                            primitive += e_x[t as usize]
                                                * e_y[u as usize]
                                                * e_z[v as usize]
                                                * coulomb_auxiliary(t, u, v, 0, alpha, &pc);
                                        }
                                    }
                                }
                                integral -= z * prefactor * (2.0 * PI / alpha) * primitive;
                            }
                        }
                    }
                }
            }
            (i, j, integral)
        })
        .collect::<Vec<_>>();

    for (i, j, integral) in values {
        result[(i, j)] = integral;
        result[(j, i)] = integral; // Symmetry
    }

    result
}

pub fn kinetic_ints(_mol: &Geometry, basis: &Basis) -> DMatrix<f64> {
    basis.kinetic_ints()
}

pub fn core_hamiltonian_ints(mol: &Geometry, basis: &Basis) -> (DMatrix<f64>, DMatrix<f64>) {
    (kinetic_ints(mol, basis), nucl_attraction_ints(mol, basis))
}
