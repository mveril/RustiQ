// basis.rs
#![allow(non_snake_case)]

use nalgebra::{DMatrix, Vector3};
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

                                let s_xyz = compute_overlap_3d(
                                    l_i, l_j, &origin_i, &origin_j, exp_i, exp_j,
                                );

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

                                let t_xyz = compute_kinetic_3d(
                                    l_i, l_j, &origin_i, &origin_j, exp_i, exp_j,
                                );

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

fn compute_overlap_3d(
    l_i: &Vector3<u8>,
    l_j: &Vector3<u8>,
    origin_i: &Vector3<f64>,
    origin_j: &Vector3<f64>,
    exp_i: f64,
    exp_j: f64,
) -> f64 {
    let la_x = l_i.x as i32;
    let lb_x = l_j.x as i32;
    let la_y = l_i.y as i32;
    let lb_y = l_j.y as i32;
    let la_z = l_i.z as i32;
    let lb_z = l_j.z as i32;
    let p = exp_i + exp_j;
    let reduced_exp = (exp_i * exp_j) / p;
    let P = (exp_i * origin_i + exp_j * origin_j) / p;
    let PA = P - origin_i;
    let PB = P - origin_j;
    let d_sq = (origin_i - origin_j).norm_squared();

    // Calcul des intégrales 1D pour chaque axe
    let s_x = overlap_1d(la_x, lb_x, PA.x, PB.x, p);
    let s_y = overlap_1d(la_y, lb_y, PA.y, PB.y, p);
    let s_z = overlap_1d(la_z, lb_z, PA.z, PB.z, p);

    // Produit des intégrales 1D
    (-reduced_exp * d_sq).exp() * s_x * s_y * s_z
}

fn compute_kinetic_3d(
    l_i: &Vector3<u8>,
    l_j: &Vector3<u8>,
    origin_i: &Vector3<f64>,
    origin_j: &Vector3<f64>,
    exp_i: f64,
    exp_j: f64,
) -> f64 {
    if l_i == &Vector3::new(0, 0, 0) && l_j == &Vector3::new(0, 0, 0) {
        let p = exp_i + exp_j;
        let reduced_exp = exp_i * exp_j / p;
        let r2 = (origin_i - origin_j).norm_squared();
        let overlap = (PI / p).powf(1.5) * (-reduced_exp * r2).exp();
        return reduced_exp * (3.0 - 2.0 * reduced_exp * r2) * overlap;
    }

    let gamma = exp_i + exp_j;
    let p = gamma;
    let P = (exp_i * origin_i + exp_j * origin_j) / p;
    let PAx = P.x - origin_i.x;
    let PAy = P.y - origin_i.y;
    let PAz = P.z - origin_i.z;
    let PBx = P.x - origin_j.x;
    let PBy = P.y - origin_j.y;
    let PBz = P.z - origin_j.z;

    let la_x = l_i.x as i32;
    let lb_x = l_j.x as i32;
    let la_y = l_i.y as i32;
    let lb_y = l_j.y as i32;
    let la_z = l_i.z as i32;
    let lb_z = l_j.z as i32;

    // Calcul des intégrales de recouvrement 1D
    let s_x = overlap_1d(la_x, lb_x, PAx, PBx, gamma);
    let s_y = overlap_1d(la_y, lb_y, PAy, PBy, gamma);
    let s_z = overlap_1d(la_z, lb_z, PAz, PBz, gamma);

    // Calcul des intégrales cinétiques 1D
    let t_x = kinetic_1d(la_x, lb_x, PAx, PBx, gamma, exp_i, exp_j);
    let t_y = kinetic_1d(la_y, lb_y, PAy, PBy, gamma, exp_i, exp_j);
    let t_z = kinetic_1d(la_z, lb_z, PAz, PBz, gamma, exp_i, exp_j);

    // Combinaison pour obtenir l'intégrale cinétique 3D
    t_x * s_y * s_z + s_x * t_y * s_z + s_x * s_y * t_z
}

fn overlap_1d(la: i32, lb: i32, PAx: f64, PBx: f64, gamma: f64) -> f64 {
    let mut sum = 0.0;

    for i in 0..=la {
        for j in 0..=lb {
            let order = i + j;
            if order % 2 != 0 {
                continue;
            }

            let moment = if order == 0 {
                1.0
            } else {
                let double_factorial = (order as u64 - 1).double_factorial() as f64;
                double_factorial / (2.0 * gamma).powi(order / 2)
            };

            sum += binomial_coefficient(la as u32, i as u32)
                * binomial_coefficient(lb as u32, j as u32)
                * PAx.powi(la - i)
                * PBx.powi(lb - j)
                * moment;
        }
    }

    (PI / gamma).sqrt() * sum
}

fn kinetic_1d(la: i32, lb: i32, PAx: f64, PBx: f64, gamma: f64, exp_a: f64, exp_b: f64) -> f64 {
    let overlap = overlap_1d(la, lb, PAx, PBx, gamma);
    let pre_factor = (exp_a * exp_b) / gamma;
    let term = pre_factor * (2.0 * (la + lb) as f64 + 3.0);
    term * overlap
}

/// Calcul du coefficient binomial (n sur k)
fn binomial_coefficient(n: u32, k: u32) -> f64 {
    if k > n {
        0.0
    } else if k == 0 || k == n {
        1.0
    } else {
        let mut result = 1.0;
        for i in 1..=k {
            result *= (n - k + i) as f64 / i as f64;
        }
        result
    }
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
        let PAx = 0.0;
        let PBx = 0.0;
        let gamma = 1.0;

        let computed = overlap_1d(la, lb, PAx, PBx, gamma);

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
        let la = 0;
        let lb = 0;
        let PAx = 0.0;
        let PBx = 0.0;
        let exp_a = 0.5;
        let exp_b = 0.5;
        let gamma = exp_a + exp_b;

        let computed = kinetic_1d(la, lb, PAx, PBx, gamma, exp_a, exp_b);
        let expected = 0.75 * (PI).sqrt(); // Inclure le facteur sqrt(PI)

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
