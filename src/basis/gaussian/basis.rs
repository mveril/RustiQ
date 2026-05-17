// basis.rs
#![allow(non_snake_case)]

use nalgebra::{DMatrix, Point3, Vector3};
use std::f64::consts::PI;

use super::contraction::Contraction;
use super::shell::Shell;
use factorial::DoubleFactorial;

use crate::basis::basisfile::BasisFile;
use crate::basis::function_type::FunctionType;
use crate::molecules::geometry::Geometry;

/// Structure représentant une base de fonctions gaussiennes.
#[derive(PartialEq, Debug)]
pub struct Basis {
    pub shells: Vec<Shell>,                // Collection de shells gaussiens
    pub shell_ids: Vec<usize>,             // Indices des shells associés à chaque fonction de base
    pub angular_momenta: Vec<Vector3<u8>>, // Moments angulaires des fonctions de base
}

impl Basis {
    pub fn new(shells: Vec<Shell>) -> Self {
        let mut shell_ids = Vec::new();
        let mut angular_momenta = Vec::new();

        for (shell_index, shell) in shells.iter().enumerate() {
            for contraction in &shell.contr {
                let l = contraction.l;

                let ang_moments = generate_angular_momentum_combinations_vector(l);

                for ang_mom in ang_moments {
                    shell_ids.push(shell_index);
                    angular_momenta.push(ang_mom);
                }
            }
        }

        Self {
            shells,
            shell_ids,
            angular_momenta,
        }
    }

    /// Charge une base à partir d'un [BasisFile] et associe avec les atomes de la molécule.
    pub fn load(basis_file: &BasisFile, mol: &Geometry) -> Self {
        let mut shells = Vec::new();
        for atom in &mol.atoms {
            let element = &atom.element;
            let position = atom.position;

            if let Some(electron_shells) = basis_file
                .elements
                .get(&element.atomic_number)
                .map(|e| &e.electron_shells)
            {
                for shell in electron_shells {
                    let alpha = &shell.exponents;
                    let coeffs_list = &shell.coefficients;
                    let angular_momenta = &shell.angular_momentum;

                    // Vérifiez que le nombre de moments angulaires correspond au nombre de groupes de coefficients
                    assert_eq!(
                        angular_momenta.len(),
                        coeffs_list.len(),
                        "Mismatch between angular_momentum.len() ({}) and coefficients.len() ({}) for element {}",
                        angular_momenta.len(),
                        coeffs_list.len(),
                        element.atomic_number
                    );

                    // Pour chaque moment angulaire et son groupe de coefficients
                    for (l, coeffs_group) in angular_momenta.iter().zip(coeffs_list.iter()) {
                        // coeffs_group est un Vec<f64> (un groupe de coefficients)
                        assert_eq!(
                            alpha.len(),
                            coeffs_group.len(),  // On compare maintenant la longueur du groupe de coefficients
                            "Mismatch between exponents.len() ({}) and coeffs.len() ({}) for element {}, l = {}",
                            alpha.len(),
                            coeffs_group.len(),
                            element.atomic_number,
                            l
                        );

                        let contraction = Contraction::new(
                            *l,
                            shell.function_type == FunctionType::GtoSpherical,
                            coeffs_group.clone(), // Utilisez le groupe de coefficients complet ici
                        );

                        // Ajoutez la shell
                        shells.push(Shell::new(alpha.clone(), vec![contraction], position));
                    }
                }
            } else {
                panic!(
                    "No electron shells found for element {}",
                    element.atomic_number
                );
            }
        }
        Self::new(shells)
    }

    pub fn overlap_ints(&self) -> DMatrix<f64> {
        let n = self.angular_momenta.len();
        let mut result = DMatrix::<f64>::zeros(n, n);

        for i in 0..n {
            let shell_i = &self.shells[self.shell_ids[i]];
            let l_i = &self.angular_momenta[i];
            let origin_i = shell_i.origin.coords;

            for j in 0..=i {
                let shell_j = &self.shells[self.shell_ids[j]];
                let l_j = &self.angular_momenta[j];
                let origin_j = shell_j.origin.coords;

                let mut s_ij = 0.0;

                for contraction_i in &shell_i.contr {
                    for contraction_j in &shell_j.contr {
                        for (&exp_i, &coeff_i) in
                            shell_i.alpha.iter().zip(contraction_i.coeff.iter())
                        {
                            for (&exp_j, &coeff_j) in
                                shell_j.alpha.iter().zip(contraction_j.coeff.iter())
                            {
                                let norm_i = gaussian_norm_const(
                                    exp_i,
                                    l_i.x as u32,
                                    l_i.y as u32,
                                    l_i.z as u32,
                                );
                                let norm_j = gaussian_norm_const(
                                    exp_j,
                                    l_j.x as u32,
                                    l_j.y as u32,
                                    l_j.z as u32,
                                );

                                let s_xyz =
                                    primitive_overlap(l_i, l_j, &origin_i, &origin_j, exp_i, exp_j);

                                s_ij += coeff_i * coeff_j * norm_i * norm_j * s_xyz;
                            }
                        }
                    }
                }

                result[(i, j)] = s_ij;
                if i != j {
                    result[(j, i)] = s_ij;
                }
            }
        }

        result
    }

    pub fn kinetic_ints(&self) -> DMatrix<f64> {
        let n = self.angular_momenta.len();
        let mut result = DMatrix::<f64>::zeros(n, n);

        for i in 0..n {
            let shell_i = &self.shells[self.shell_ids[i]];
            let l_i = &self.angular_momenta[i];
            let origin_i = shell_i.origin.coords;

            for j in 0..=i {
                let shell_j = &self.shells[self.shell_ids[j]];
                let l_j = &self.angular_momenta[j];
                let origin_j = shell_j.origin.coords;

                let mut t_ij = 0.0;

                for contraction_i in &shell_i.contr {
                    for contraction_j in &shell_j.contr {
                        for (&exp_i, &coeff_i) in
                            shell_i.alpha.iter().zip(contraction_i.coeff.iter())
                        {
                            for (&exp_j, &coeff_j) in
                                shell_j.alpha.iter().zip(contraction_j.coeff.iter())
                            {
                                let norm_i = gaussian_norm_const(
                                    exp_i,
                                    l_i.x as u32,
                                    l_i.y as u32,
                                    l_i.z as u32,
                                );
                                let norm_j = gaussian_norm_const(
                                    exp_j,
                                    l_j.x as u32,
                                    l_j.y as u32,
                                    l_j.z as u32,
                                );

                                let t_xyz =
                                    primitive_kinetic(l_i, l_j, &origin_i, &origin_j, exp_i, exp_j);

                                t_ij += coeff_i * coeff_j * norm_i * norm_j * t_xyz;
                            }
                        }
                    }
                }

                result[(i, j)] = t_ij;
                if i != j {
                    result[(j, i)] = t_ij;
                }
            }
        }

        result
    }

    /// Renvoie le nombre total de fonctions de base.
    pub fn nbasis(&self) -> usize {
        self.angular_momenta.len()
    }
}

/// Génère les combinaisons de moments angulaires pour un l donné.
fn generate_angular_momentum_combinations_vector(l: u8) -> Vec<Vector3<u8>> {
    let mut combinations = Vec::new();
    for lx in 0..=l {
        for ly in 0..=(l - lx) {
            let lz = l - lx - ly;
            combinations.push(Vector3::new(lx, ly, lz));
        }
    }
    combinations
}

pub fn gaussian_norm_const(alpha: f64, l: u32, m: u32, n: u32) -> f64 {
    let l_factor = if l == 0 {
        1.0
    } else {
        (2 * l - 1).double_factorial() as f64
    };
    let m_factor = if m == 0 {
        1.0
    } else {
        (2 * m - 1).double_factorial() as f64
    };
    let n_factor = if n == 0 {
        1.0
    } else {
        (2 * n - 1).double_factorial() as f64
    };

    ((2.0 * alpha / PI).powf(0.75)) * (4.0 * alpha).powf((l + m + n) as f64 / 2.0)
        / ((l_factor * m_factor * n_factor).sqrt())
}

pub(crate) fn primitive_overlap(
    l_i: &Vector3<u8>,
    l_j: &Vector3<u8>,
    origin_i: &Vector3<f64>,
    origin_j: &Vector3<f64>,
    exp_i: f64,
    exp_j: f64,
) -> f64 {
    let p = exp_i + exp_j;
    (PI / p).powf(1.5)
        * hermite_coeff(l_i.x, l_j.x, 0, origin_i.x - origin_j.x, exp_i, exp_j)
        * hermite_coeff(l_i.y, l_j.y, 0, origin_i.y - origin_j.y, exp_i, exp_j)
        * hermite_coeff(l_i.z, l_j.z, 0, origin_i.z - origin_j.z, exp_i, exp_j)
}

pub(crate) fn primitive_kinetic(
    l_i: &Vector3<u8>,
    l_j: &Vector3<u8>,
    origin_i: &Vector3<f64>,
    origin_j: &Vector3<f64>,
    exp_i: f64,
    exp_j: f64,
) -> f64 {
    let l_b = [l_j.x as i32, l_j.y as i32, l_j.z as i32];
    let s = |dx: i32, dy: i32, dz: i32| {
        let shifted = Vector3::new(
            (l_b[0] + dx).max(0) as u8,
            (l_b[1] + dy).max(0) as u8,
            (l_b[2] + dz).max(0) as u8,
        );
        primitive_overlap(l_i, &shifted, origin_i, origin_j, exp_i, exp_j)
    };

    let mut result = exp_j * (2.0 * (l_b[0] + l_b[1] + l_b[2]) as f64 + 3.0) * s(0, 0, 0);
    result -= 2.0 * exp_j.powi(2) * (s(2, 0, 0) + s(0, 2, 0) + s(0, 0, 2));

    if l_b[0] >= 2 {
        result -= 0.5 * (l_b[0] * (l_b[0] - 1)) as f64 * s(-2, 0, 0);
    }
    if l_b[1] >= 2 {
        result -= 0.5 * (l_b[1] * (l_b[1] - 1)) as f64 * s(0, -2, 0);
    }
    if l_b[2] >= 2 {
        result -= 0.5 * (l_b[2] * (l_b[2] - 1)) as f64 * s(0, 0, -2);
    }
    result
}

pub(crate) fn gaussian_product_center(
    exp_i: f64,
    origin_i: &Point3<f64>,
    exp_j: f64,
    origin_j: &Point3<f64>,
) -> Vector3<f64> {
    (exp_i * origin_i.coords + exp_j * origin_j.coords) / (exp_i + exp_j)
}

pub(crate) fn hermite_coeff(i: u8, j: u8, t: u8, qx: f64, a: f64, b: f64) -> f64 {
    if t > i + j {
        return 0.0;
    }
    if i == 0 && j == 0 && t == 0 {
        let p = a + b;
        let reduced_exp = a * b / p;
        return (-reduced_exp * qx.powi(2)).exp();
    }
    if i == 0 && j == 0 {
        return 0.0;
    }

    let p = a + b;
    let reduced_exp = a * b / p;
    if i > 0 {
        let lower_i = i - 1;
        let left = if t > 0 {
            hermite_coeff(lower_i, j, t - 1, qx, a, b) / (2.0 * p)
        } else {
            0.0
        };
        let middle = -(reduced_exp * qx / a) * hermite_coeff(lower_i, j, t, qx, a, b);
        let right = (t as f64 + 1.0) * hermite_coeff(lower_i, j, t + 1, qx, a, b);
        left + middle + right
    } else {
        let lower_j = j - 1;
        let left = if t > 0 {
            hermite_coeff(i, lower_j, t - 1, qx, a, b) / (2.0 * p)
        } else {
            0.0
        };
        let middle = (reduced_exp * qx / b) * hermite_coeff(i, lower_j, t, qx, a, b);
        let right = (t as f64 + 1.0) * hermite_coeff(i, lower_j, t + 1, qx, a, b);
        left + middle + right
    }
}

pub(crate) fn coulomb_auxiliary(t: u8, u: u8, v: u8, n: u8, p: f64, pc: &Vector3<f64>) -> f64 {
    if t == 0 && u == 0 && v == 0 {
        return (-2.0 * p).powi(n as i32)
            * crate::math_utils::boys_function(n as u64, p * pc.norm_squared());
    }
    if t > 0 {
        let lower = if t >= 2 {
            (t as f64 - 1.0) * coulomb_auxiliary(t - 2, u, v, n + 1, p, pc)
        } else {
            0.0
        };
        return lower + pc.x * coulomb_auxiliary(t - 1, u, v, n + 1, p, pc);
    }
    if u > 0 {
        let lower = if u >= 2 {
            (u as f64 - 1.0) * coulomb_auxiliary(t, u - 2, v, n + 1, p, pc)
        } else {
            0.0
        };
        return lower + pc.y * coulomb_auxiliary(t, u - 1, v, n + 1, p, pc);
    }
    let lower = if v >= 2 {
        (v as f64 - 1.0) * coulomb_auxiliary(t, u, v - 2, n + 1, p, pc)
    } else {
        0.0
    };
    lower + pc.z * coulomb_auxiliary(t, u, v - 1, n + 1, p, pc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        molecules::{atom::Atom, units::Units},
        test_utils,
    };
    use nalgebra::*;
    use periodic_table::periodic_table;

    #[test]
    fn test_basis_new() {
        let alpha = vec![0.5];
        let contr = vec![Contraction::new(0, false, vec![1.0])];
        let origin = Point3::origin();
        let shell = Shell::new(alpha.clone(), contr.clone(), origin);
        let basis = Basis::new(vec![shell.clone()]);

        assert_eq!(basis.shells.len(), 1);
        assert_eq!(basis.shell_ids.len(), 1); // Il y a une seule fonction de base
        assert_eq!(basis.angular_momenta.len(), 1);
        assert_eq!(basis.angular_momenta[0], Vector3::new(0, 0, 0));
    }

    #[test]
    fn test_overlap_1d_simple() {
        let la = 0;
        let lb = 0;
        let gamma = 1.0;

        let computed = hermite_coeff(la, lb, 0, 0.0, 0.5, 0.5) * (PI / gamma).sqrt();

        // L'intégrale de recouvrement devrait être (pi / gamma)^0.5
        let expected = (PI / gamma).sqrt();
        assert!(
            (computed - expected).abs() < 1e-6,
            "Overlap_1d simple: attendu {}, obtenu {}",
            expected,
            computed
        );
    }

    #[test]
    fn test_kinetic_1d_simple() {
        let exp_a = 0.5;
        let exp_b = 0.5;

        let computed = primitive_kinetic(
            &Vector3::new(0, 0, 0),
            &Vector3::new(0, 0, 0),
            &Vector3::zeros(),
            &Vector3::zeros(),
            exp_a,
            exp_b,
        );
        let expected = 0.75 * PI.powf(1.5);

        assert!(
            (computed - expected).abs() < 1e-6,
            "Kinetic_1d simple: attendu {}, obtenu {}",
            expected,
            computed
        );
    }

    #[test]
    fn test_basis_hydrogen_minimal() {
        let basis_file = test_utils::load_minimal_basis_file();
        let element = periodic_table()[0];
        let atom = Atom {
            element,
            position: Point3::origin(),
        };
        let geom = Geometry {
            comment: "Hydrogen Atom".to_string(),
            atoms: vec![atom],
            display_unit: Units::Bohr,
            unit: Units::Bohr,
        };

        let basis = Basis::load(&basis_file, &geom);

        // Vérifier les intégrales de recouvrement et cinétiques
        let overlap = basis.overlap_ints();
        let kinetic = basis.kinetic_ints();

        assert!(overlap[(0, 0)] > 0.0, "Overlap(0,0) devrait être positif");
        assert!(kinetic[(0, 0)] > 0.0, "Kinetic(0,0) devrait être positif");
    }

    #[test]
    fn test_overlap_matrix_symmetry() {
        let alpha = vec![0.5];
        let contr = vec![Contraction::new(0, false, vec![1.0])];
        let origin = point!(0.0, 0.0, 0.0);
        let shell = Shell::new(alpha.clone(), contr.clone(), origin);
        let basis = Basis::new(vec![shell.clone(), shell.clone()]);

        let overlap_matrix = basis.overlap_ints();
        let n = overlap_matrix.nrows();

        for i in 0..n {
            for j in 0..n {
                assert!(
                    (overlap_matrix[(i, j)] - overlap_matrix[(j, i)]).abs() < 1e-12,
                    "La matrice n'est pas symétrique : S[{},{}] = {}, mais S[{},{}] = {}",
                    i,
                    j,
                    overlap_matrix[(i, j)],
                    j,
                    i,
                    overlap_matrix[(j, i)]
                );
            }
        }
    }
}
