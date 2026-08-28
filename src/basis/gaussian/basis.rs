// basis.rs
#![allow(non_snake_case)]

use nalgebra::{DMatrix, Point3, Vector3};
use std::f64::consts::PI;

use super::contraction::Contraction;
use super::shell::Shell;
use factorial::DoubleFactorial;
use rayon::prelude::*;
use thiserror::Error;

use crate::basis::basis_file::BasisFile;
use crate::basis::function_type::FunctionType;
use crate::molecules::geometry::Geometry;

/// Structure representing a Gaussian basis set.
#[derive(PartialEq, Debug)]
pub struct Basis {
    pub shells: Vec<Shell>,                // Collection of Gaussian shells
    pub shell_ids: Vec<usize>,             // Shell indices associated with each basis function
    pub angular_momenta: Vec<Vector3<u8>>, // Angular momenta of the basis functions
    pub angular_components: Vec<Vec<(Vector3<u8>, f64)>>,
    pub normalized_components: Vec<Vec<NormalizedComponent>>,
}

/// An error raised before any basis functions are constructed.
#[derive(Debug, Error, PartialEq)]
pub enum BasisError {
    #[error("basis file declares unsupported function type {function_type:?}")]
    UnsupportedDeclaredFunctionType { function_type: FunctionType },
    #[error("basis data for element Z={atomic_number} contains an ECP ({ecp_electrons} core electrons, {potential_count} potentials), but RustiQ does not support ECPs")]
    UnsupportedEcp {
        atomic_number: u32,
        ecp_electrons: usize,
        potential_count: usize,
    },
    #[error("electron shell {shell_index} for element Z={atomic_number} uses unsupported function type {function_type:?}")]
    UnsupportedFunctionType {
        atomic_number: u32,
        shell_index: usize,
        function_type: FunctionType,
    },
    #[error("electron shell {shell_index} for element Z={atomic_number} has no angular momenta")]
    EmptyAngularMomenta {
        atomic_number: u32,
        shell_index: usize,
    },
    #[error("electron shell {shell_index} for element Z={atomic_number} has no exponents")]
    EmptyExponents {
        atomic_number: u32,
        shell_index: usize,
    },
    #[error("electron shell {shell_index} for element Z={atomic_number} has {angular_momenta} angular momenta but {coefficient_groups} coefficient groups")]
    ContractionCountMismatch {
        atomic_number: u32,
        shell_index: usize,
        angular_momenta: usize,
        coefficient_groups: usize,
    },
    #[error("coefficient group {coefficient_group} in electron shell {shell_index} for element Z={atomic_number} has length {coefficients}, expected {exponents}")]
    PrimitiveCountMismatch {
        atomic_number: u32,
        shell_index: usize,
        coefficient_group: usize,
        exponents: usize,
        coefficients: usize,
    },
    #[error("electron shell {shell_index} for element Z={atomic_number} contains an invalid exponent at index {exponent_index}: {value} (expected a finite positive number)")]
    InvalidExponent {
        atomic_number: u32,
        shell_index: usize,
        exponent_index: usize,
        value: f64,
    },
    #[error("coefficient group {coefficient_group} in electron shell {shell_index} for element Z={atomic_number} contains a non-finite coefficient at index {coefficient_index}: {value}")]
    InvalidCoefficient {
        atomic_number: u32,
        shell_index: usize,
        coefficient_group: usize,
        coefficient_index: usize,
        value: f64,
    },
    #[error("coefficient group {coefficient_group} in electron shell {shell_index} for element Z={atomic_number} is identically zero")]
    ZeroContraction {
        atomic_number: u32,
        shell_index: usize,
        coefficient_group: usize,
    },
    #[error("electron shell {shell_index} for element Z={atomic_number} uses unsupported angular momentum l={angular_momentum} for {function_type:?}")]
    UnsupportedAngularMomentum {
        atomic_number: u32,
        shell_index: usize,
        angular_momentum: u8,
        function_type: FunctionType,
    },
    #[error(
        "basis file has no electron shells for element Z={atomic_number} required by the molecule"
    )]
    MissingElement { atomic_number: u32 },
}

#[derive(Clone, PartialEq, Debug)]
pub struct NormalizedComponent {
    pub angular_momentum: Vector3<u8>,
    pub primitives: Vec<NormalizedPrimitive>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct NormalizedPrimitive {
    pub exponent: f64,
    pub coefficient: f64,
}

impl Basis {
    pub fn new(shells: Vec<Shell>) -> Self {
        let mut shell_ids = Vec::new();
        let mut angular_momenta = Vec::new();
        let mut angular_components = Vec::new();

        for (shell_index, shell) in shells.iter().enumerate() {
            for contraction in &shell.contr {
                let l = contraction.l;

                for components in generate_angular_components(l, contraction.pure) {
                    shell_ids.push(shell_index);
                    angular_momenta.push(components[0].0);
                    angular_components.push(components);
                }
            }
        }

        let normalized_components =
            build_normalized_components(&shells, &shell_ids, &angular_components);

        Self {
            shells,
            shell_ids,
            angular_momenta,
            angular_components,
            normalized_components,
        }
    }

    /// Validates and loads a basis, without constructing anything on invalid input.
    pub fn try_load(basis_file: &BasisFile, mol: &Geometry) -> Result<Self, BasisError> {
        Self::validate(basis_file, mol)?;
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

                    let angular_coeff_pairs: Vec<_> = if angular_momenta.len() == coeffs_list.len()
                    {
                        angular_momenta.iter().zip(coeffs_list.iter()).collect()
                    } else if angular_momenta.len() == 1 {
                        coeffs_list
                            .iter()
                            .map(|coeffs_group| (&angular_momenta[0], coeffs_group))
                            .collect()
                    } else {
                        // Guaranteed unreachable by validate().
                        unreachable!("validated contraction dimensions")
                    };

                    for (l, coeffs_group) in angular_coeff_pairs {
                        // coeffs_group is a Vec<f64> (a coefficient group)
                        let contraction = Contraction::new(
                            *l,
                            shell.function_type == FunctionType::GtoSpherical,
                            coeffs_group.clone(), // Use the full coefficient group here
                        );

                        // Add the shell
                        shells.push(Shell::new(alpha.clone(), vec![contraction], position));
                    }
                }
            }
        }
        Ok(Self::new(shells))
    }

    fn validate(basis_file: &BasisFile, mol: &Geometry) -> Result<(), BasisError> {
        if let Some(function_type) = basis_file.function_types.iter().find(|function_type| {
            !matches!(
                function_type,
                FunctionType::Gto | FunctionType::GtoCartesian | FunctionType::GtoSpherical
            )
        }) {
            return Err(BasisError::UnsupportedDeclaredFunctionType {
                function_type: *function_type,
            });
        }
        for (&atomic_number, element) in &basis_file.elements {
            if element.ecp_electrons != 0 || !element.ecp_potentials.is_empty() {
                return Err(BasisError::UnsupportedEcp {
                    atomic_number,
                    ecp_electrons: element.ecp_electrons,
                    potential_count: element.ecp_potentials.len(),
                });
            }
            for (shell_index, shell) in element.electron_shells.iter().enumerate() {
                if !matches!(
                    shell.function_type,
                    FunctionType::Gto | FunctionType::GtoCartesian | FunctionType::GtoSpherical
                ) {
                    return Err(BasisError::UnsupportedFunctionType {
                        atomic_number,
                        shell_index,
                        function_type: shell.function_type,
                    });
                }
                if shell.angular_momentum.is_empty() {
                    return Err(BasisError::EmptyAngularMomenta {
                        atomic_number,
                        shell_index,
                    });
                }
                if shell.exponents.is_empty() {
                    return Err(BasisError::EmptyExponents {
                        atomic_number,
                        shell_index,
                    });
                }
                let counts_match = shell.angular_momentum.len() == shell.coefficients.len()
                    || (shell.angular_momentum.len() == 1 && !shell.coefficients.is_empty());
                if !counts_match {
                    return Err(BasisError::ContractionCountMismatch {
                        atomic_number,
                        shell_index,
                        angular_momenta: shell.angular_momentum.len(),
                        coefficient_groups: shell.coefficients.len(),
                    });
                }
                for (exponent_index, &value) in shell.exponents.iter().enumerate() {
                    if !value.is_finite() || value <= 0.0 {
                        return Err(BasisError::InvalidExponent {
                            atomic_number,
                            shell_index,
                            exponent_index,
                            value,
                        });
                    }
                }
                for (coefficient_group, coefficients) in shell.coefficients.iter().enumerate() {
                    if coefficients.len() != shell.exponents.len() {
                        return Err(BasisError::PrimitiveCountMismatch {
                            atomic_number,
                            shell_index,
                            coefficient_group,
                            exponents: shell.exponents.len(),
                            coefficients: coefficients.len(),
                        });
                    }
                    for (coefficient_index, &value) in coefficients.iter().enumerate() {
                        if !value.is_finite() {
                            return Err(BasisError::InvalidCoefficient {
                                atomic_number,
                                shell_index,
                                coefficient_group,
                                coefficient_index,
                                value,
                            });
                        }
                    }
                    if coefficients.iter().all(|&value| value == 0.0) {
                        return Err(BasisError::ZeroContraction {
                            atomic_number,
                            shell_index,
                            coefficient_group,
                        });
                    }
                }
                for &angular_momentum in &shell.angular_momentum {
                    let supported = if shell.function_type == FunctionType::GtoSpherical {
                        angular_momentum <= 2
                    } else {
                        angular_momentum <= 15
                    };
                    if !supported {
                        return Err(BasisError::UnsupportedAngularMomentum {
                            atomic_number,
                            shell_index,
                            angular_momentum,
                            function_type: shell.function_type,
                        });
                    }
                }
            }
        }
        for atom in &mol.atoms {
            let atomic_number = atom.element.atomic_number;
            if !basis_file
                .elements
                .get(&atomic_number)
                .is_some_and(|element| !element.electron_shells.is_empty())
            {
                return Err(BasisError::MissingElement { atomic_number });
            }
        }
        Ok(())
    }

    pub fn overlap_ints(&self) -> DMatrix<f64> {
        let n = self.angular_momenta.len();
        let mut result = DMatrix::<f64>::zeros(n, n);

        let values = (0..n * n)
            .into_par_iter()
            .filter_map(|index| {
                let i = index / n;
                let j = index % n;
                (j <= i).then_some((i, j))
            })
            .map(|(i, j)| {
                let shell_i = &self.shells[self.shell_ids[i]];
                let origin_i = shell_i.origin;
                let shell_j = &self.shells[self.shell_ids[j]];
                let origin_j = shell_j.origin;

                let mut s_ij = 0.0;

                for component_i in &self.normalized_components[i] {
                    for component_j in &self.normalized_components[j] {
                        for primitive_i in &component_i.primitives {
                            for primitive_j in &component_j.primitives {
                                let s_xyz = primitive_overlap(
                                    &component_i.angular_momentum,
                                    &component_j.angular_momentum,
                                    &origin_i,
                                    &origin_j,
                                    primitive_i.exponent,
                                    primitive_j.exponent,
                                );

                                s_ij += primitive_i.coefficient * primitive_j.coefficient * s_xyz;
                            }
                        }
                    }
                }

                (i, j, s_ij)
            })
            .collect::<Vec<_>>();

        for (i, j, s_ij) in values {
            result[(i, j)] = s_ij;
            if i != j {
                result[(j, i)] = s_ij;
            }
        }

        result
    }

    pub fn kinetic_ints(&self) -> DMatrix<f64> {
        let n: usize = self.angular_momenta.len();
        let mut result = DMatrix::<f64>::zeros(n, n);

        let values = (0..n * n)
            .into_par_iter()
            .filter_map(|index| {
                let i = index / n;
                let j = index % n;
                (j <= i).then_some((i, j))
            })
            .map(|(i, j)| {
                let shell_i = &self.shells[self.shell_ids[i]];
                let origin_i = shell_i.origin;
                let shell_j = &self.shells[self.shell_ids[j]];
                let origin_j = shell_j.origin;

                let mut t_ij = 0.0;

                for component_i in &self.normalized_components[i] {
                    for component_j in &self.normalized_components[j] {
                        for primitive_i in &component_i.primitives {
                            for primitive_j in &component_j.primitives {
                                let t_xyz = primitive_kinetic(
                                    &component_i.angular_momentum,
                                    &component_j.angular_momentum,
                                    &origin_i,
                                    &origin_j,
                                    primitive_i.exponent,
                                    primitive_j.exponent,
                                );

                                t_ij += primitive_i.coefficient * primitive_j.coefficient * t_xyz;
                            }
                        }
                    }
                }

                (i, j, t_ij)
            })
            .collect::<Vec<_>>();

        for (i, j, t_ij) in values {
            result[(i, j)] = t_ij;
            if i != j {
                result[(j, i)] = t_ij;
            }
        }

        result
    }

    /// Returns the total number of basis functions.
    pub fn nbasis(&self) -> usize {
        self.angular_momenta.len()
    }
}

fn build_normalized_components(
    shells: &[Shell],
    shell_ids: &[usize],
    angular_components: &[Vec<(Vector3<u8>, f64)>],
) -> Vec<Vec<NormalizedComponent>> {
    shell_ids
        .iter()
        .zip(angular_components.iter())
        .map(|(&shell_id, components)| {
            let shell = &shells[shell_id];
            components
                .iter()
                .map(|&(angular_momentum, component_factor)| {
                    let primitives = shell
                        .contr
                        .iter()
                        .flat_map(|contraction| {
                            shell.alpha.iter().zip(contraction.coeff.iter()).map(
                                move |(&exponent, &coefficient)| {
                                    let norm = gaussian_norm_const(
                                        exponent,
                                        angular_momentum.x as u32,
                                        angular_momentum.y as u32,
                                        angular_momentum.z as u32,
                                    );
                                    NormalizedPrimitive {
                                        exponent,
                                        coefficient: component_factor * coefficient * norm,
                                    }
                                },
                            )
                        })
                        .collect();
                    NormalizedComponent {
                        angular_momentum,
                        primitives,
                    }
                })
                .collect()
        })
        .collect()
}

/// Generates the angular momentum combinations for a given l.
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

fn generate_angular_components(l: u8, pure: bool) -> Vec<Vec<(Vector3<u8>, f64)>> {
    if !pure || l <= 1 {
        return generate_angular_momentum_combinations_vector(l)
            .into_iter()
            .map(|angular_momentum| vec![(angular_momentum, 1.0)])
            .collect();
    }

    match l {
        2 => vec![
            vec![(Vector3::new(1, 1, 0), 1.0)],
            vec![(Vector3::new(0, 1, 1), 1.0)],
            vec![
                (Vector3::new(2, 0, 0), -0.5),
                (Vector3::new(0, 2, 0), -0.5),
                (Vector3::new(0, 0, 2), 1.0),
            ],
            vec![(Vector3::new(1, 0, 1), 1.0)],
            vec![
                (Vector3::new(2, 0, 0), 3.0_f64.sqrt() / 2.0),
                (Vector3::new(0, 2, 0), -3.0_f64.sqrt() / 2.0),
            ],
        ],
        _ => panic!("Spherical Gaussian functions with l > 2 are not supported yet"),
    }
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
    origin_i: &Point3<f64>,
    origin_j: &Point3<f64>,
    exp_i: f64,
    exp_j: f64,
) -> f64 {
    let p = exp_i + exp_j;
    let displacement = origin_i - origin_j;
    (PI / p).powf(1.5)
        * hermite_coeff(l_i.x, l_j.x, 0, displacement.x, exp_i, exp_j)
        * hermite_coeff(l_i.y, l_j.y, 0, displacement.y, exp_i, exp_j)
        * hermite_coeff(l_i.z, l_j.z, 0, displacement.z, exp_i, exp_j)
}

pub(crate) fn primitive_kinetic(
    l_i: &Vector3<u8>,
    l_j: &Vector3<u8>,
    origin_i: &Point3<f64>,
    origin_j: &Point3<f64>,
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
) -> Point3<f64> {
    Point3::from((exp_i * origin_i.coords + exp_j * origin_j.coords) / (exp_i + exp_j))
}

pub(crate) struct HermiteTerm {
    pub(crate) orders: Vector3<u8>,
    pub(crate) coefficient: f64,
}

pub(crate) fn hermite_terms(e: &[Vec<f64>; 3]) -> Vec<HermiteTerm> {
    let [e_x, e_y, e_z] = e;
    let mut terms = Vec::with_capacity(e_x.len() * e_y.len() * e_z.len());
    for (t, &x) in e_x.iter().enumerate() {
        for (u, &y) in e_y.iter().enumerate() {
            for (v, &z) in e_z.iter().enumerate() {
                terms.push(HermiteTerm {
                    orders: Vector3::new(t as u8, u as u8, v as u8),
                    coefficient: x * y * z,
                });
            }
        }
    }
    terms
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

pub(crate) fn coulomb_auxiliary(orders: Vector3<u8>, n: u8, p: f64, pc: &Vector3<f64>) -> f64 {
    coulomb_auxiliary_at(orders.x, orders.y, orders.z, n, p, pc)
}

fn coulomb_auxiliary_at(t: u8, u: u8, v: u8, n: u8, p: f64, pc: &Vector3<f64>) -> f64 {
    if t == 0 && u == 0 && v == 0 {
        return (-2.0 * p).powi(n as i32)
            * crate::math_utils::boys_function(n as u64, p * pc.norm_squared());
    }
    if t > 0 {
        let lower = if t >= 2 {
            (t as f64 - 1.0) * coulomb_auxiliary_at(t - 2, u, v, n + 1, p, pc)
        } else {
            0.0
        };
        return lower + pc.x * coulomb_auxiliary_at(t - 1, u, v, n + 1, p, pc);
    }
    if u > 0 {
        let lower = if u >= 2 {
            (u as f64 - 1.0) * coulomb_auxiliary_at(t, u - 2, v, n + 1, p, pc)
        } else {
            0.0
        };
        return lower + pc.y * coulomb_auxiliary_at(t, u - 1, v, n + 1, p, pc);
    }
    let lower = if v >= 2 {
        (v as f64 - 1.0) * coulomb_auxiliary_at(t, u, v - 2, n + 1, p, pc)
    } else {
        0.0
    };
    lower + pc.z * coulomb_auxiliary_at(t, u, v - 1, n + 1, p, pc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basis::basis_file::{Auxiliaries, BasisFile, MolssiBseSchema, Role, SchemaType};
    use crate::{molecules::atom::Atom, test_utils};
    use nalgebra::*;
    use periodic_table::periodic_table;
    use std::collections::{HashMap, HashSet};

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
        assert_eq!(basis.angular_momenta[0], Vector3::zeros());
    }

    #[test]
    fn test_overlap_1d_simple() {
        let la = 0;
        let lb = 0;
        let gamma = 1.0;

        let computed = hermite_coeff(la, lb, 0, 0.0, 0.5, 0.5) * (PI / gamma).sqrt();

        // The overlap integral should be (pi / gamma)^0.5
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
            &Vector3::zeros(),
            &Vector3::zeros(),
            &Point3::origin(),
            &Point3::origin(),
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
        };

        let basis = Basis::try_load(&basis_file, &geom).unwrap();

        // Check overlap and kinetic integrals
        let overlap = basis.overlap_ints();
        let kinetic = basis.kinetic_ints();

        assert!(overlap[(0, 0)] > 0.0, "Overlap(0,0) devrait être positif");
        assert!(kinetic[(0, 0)] > 0.0, "Kinetic(0,0) devrait être positif");
    }

    #[test]
    fn test_load_hydrogen_shell_with_multiple_same_angular_momentum_contractions() {
        let basis_file = BasisFile {
            auxiliaries: Auxiliaries::default(),
            description: "test".to_string(),
            elements: HashMap::from([(
                1,
                crate::basis::basis_file::Element {
                    references: Vec::new(),
                    electron_shells: vec![crate::basis::basis_file::ElectronShell {
                        function_type: FunctionType::Gto,
                        region: None,
                        angular_momentum: vec![0],
                        r_exponents: Vec::new(),
                        exponents: vec![13.01, 1.962, 0.4446, 0.122],
                        coefficients: vec![
                            vec![0.019685, 0.137977, 0.478148, 0.50124],
                            vec![0.0, 0.0, 0.0, 1.0],
                        ],
                    }],
                    ecp_electrons: 0,
                    ecp_potentials: Vec::new(),
                },
            )]),
            family: "test".to_string(),
            function_types: HashSet::from([FunctionType::Gto]),
            molssi_bse_schema: MolssiBseSchema {
                schema_type: SchemaType::Complete,
                schema_version: "0.1".to_string(),
            },
            name: "test".to_string(),
            names: Vec::new(),
            revision_date: "test".to_string(),
            revision_description: "test".to_string(),
            role: Role::Orbital,
            tags: Vec::new(),
            version: "test".to_string(),
        };
        let element = periodic_table()[0];
        let atom = Atom {
            element,
            position: Point3::origin(),
        };
        let geom = Geometry {
            comment: "Hydrogen Atom".to_string(),
            atoms: vec![atom],
        };

        let basis = Basis::try_load(&basis_file, &geom).unwrap();

        assert_eq!(basis.shells.len(), 2);
        assert_eq!(basis.nbasis(), 2);
        assert_eq!(basis.angular_momenta, vec![Vector3::zeros(); 2]);
    }

    fn hydrogen_geometry() -> Geometry {
        Geometry {
            comment: "Hydrogen Atom".to_string(),
            atoms: vec![Atom {
                element: periodic_table()[0],
                position: Point3::origin(),
            }],
        }
    }

    #[test]
    fn try_load_rejects_ecp_data() {
        let mut basis_file = test_utils::load_minimal_basis_file();
        basis_file.elements.get_mut(&1).unwrap().ecp_electrons = 2;

        assert!(matches!(
            Basis::try_load(&basis_file, &hydrogen_geometry()),
            Err(BasisError::UnsupportedEcp {
                atomic_number: 1,
                ..
            })
        ));
    }

    #[test]
    fn try_load_rejects_unsupported_function_type() {
        let mut basis_file = test_utils::load_minimal_basis_file();
        basis_file.function_types = HashSet::from([FunctionType::Sto]);

        assert_eq!(
            Basis::try_load(&basis_file, &hydrogen_geometry()),
            Err(BasisError::UnsupportedDeclaredFunctionType {
                function_type: FunctionType::Sto,
            })
        );
    }

    #[test]
    fn try_load_rejects_unsupported_spherical_angular_momentum() {
        let mut basis_file = test_utils::load_minimal_basis_file();
        let shell = &mut basis_file.elements.get_mut(&1).unwrap().electron_shells[0];
        shell.function_type = FunctionType::GtoSpherical;
        shell.angular_momentum = vec![3];

        assert!(matches!(
            Basis::try_load(&basis_file, &hydrogen_geometry()),
            Err(BasisError::UnsupportedAngularMomentum {
                atomic_number: 1,
                angular_momentum: 3,
                ..
            })
        ));
    }

    #[test]
    fn try_load_rejects_primitive_count_mismatch() {
        let mut basis_file = test_utils::load_minimal_basis_file();
        basis_file.elements.get_mut(&1).unwrap().electron_shells[0].coefficients[0].pop();

        assert!(matches!(
            Basis::try_load(&basis_file, &hydrogen_geometry()),
            Err(BasisError::PrimitiveCountMismatch {
                atomic_number: 1,
                ..
            })
        ));
    }

    #[test]
    fn test_spherical_d_shell_uses_five_transformed_functions() {
        let shell = Shell::new(
            vec![0.8],
            vec![Contraction::new(2, true, vec![1.0])],
            Point3::origin(),
        );

        let basis = Basis::new(vec![shell]);

        assert_eq!(basis.nbasis(), 5);
        assert_eq!(
            basis.angular_components[0],
            vec![(Vector3::new(1, 1, 0), 1.0)]
        );
        assert_eq!(
            basis.angular_components[1],
            vec![(Vector3::new(0, 1, 1), 1.0)]
        );
        assert_eq!(
            basis.angular_components[2],
            vec![
                (Vector3::new(2, 0, 0), -0.5),
                (Vector3::new(0, 2, 0), -0.5),
                (Vector3::new(0, 0, 2), 1.0)
            ]
        );
        assert_eq!(
            basis.angular_components[3],
            vec![(Vector3::new(1, 0, 1), 1.0)]
        );
        assert_eq!(
            basis.angular_components[4],
            vec![
                (Vector3::new(2, 0, 0), 3.0_f64.sqrt() / 2.0),
                (Vector3::new(0, 2, 0), -3.0_f64.sqrt() / 2.0)
            ]
        );
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
