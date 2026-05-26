// src/eri.rs
#![allow(non_snake_case)]

use std::f64::consts::PI;

use crate::basis::gaussian::basis::{gaussian_product_center, Basis};
use nalgebra::{Point3, Vector3};
use ndarray::Array4;
use rayon::prelude::*;

/// Computes the 1D overlap integral for two primitive Gaussian functions.
///
/// \[ S = \left( \frac{\pi}{p} \right)^{1/2} \exp\left( -\frac{\mu}{p} (A - B)^2 \right) \]
///
/// where \( p = \alpha_a + \alpha_b \) and \( \mu = \alpha_a \alpha_b \).
///
/// # Arguments
///
/// * `PAx` - x coordinate of the PA vector.
/// * `PBx` - x coordinate of the PB vector.
/// * `gamma` - \( p = \alpha_a + \alpha_b \).
///
/// # Returns
///
/// The 1D overlap integral.
#[allow(dead_code)]
pub fn overlap_1d(PAx: f64, PBx: f64, gamma: f64) -> f64 {
    let T = gamma * (PAx - PBx).powi(2);
    (PI / gamma).sqrt() * (-T).exp()
}

/// Computes the 1D kinetic integral for two primitive Gaussian functions.
///
/// \[ T = \frac{\alpha_a \alpha_b}{\gamma} (3) S \]
///
/// for s orbitals.
///
/// # Arguments
///
/// * `PAx` - x coordinate of the PA vector.
/// * `PBx` - x coordinate of the PB vector.
/// * `gamma` - \( \gamma = \alpha_a + \alpha_b \).
/// * `alpha_a` - Gaussian exponent of the first function.
/// * `alpha_b` - Gaussian exponent of the second function.
///
/// # Returns
///
/// The 1D kinetic integral.
#[allow(dead_code)]
pub fn kinetic_1d(PAx: f64, PBx: f64, gamma: f64, alpha_a: f64, alpha_b: f64) -> f64 {
    let S = overlap_1d(PAx, PBx, gamma);
    (alpha_a * alpha_b / gamma) * 3.0 * S
}

/// Computes the ERI integral for Gaussian s primitives.
///
/// \[ (ss|ss) = \frac{2 \pi^{5/2}}{p q \sqrt{p + q}} K_{ab} K_{cd} F_0(T) \]
///
/// # Arguments
///
/// * `alpha_p`, `alpha_q`, `alpha_r`, `alpha_s` - Primitive exponents.
/// * `A` - Position of center A.
/// * `B` - Position of center B.
/// * `C` - Position of center C.
/// * `D` - Position of center D.
///
/// # Returns
///
/// The value of the primitive ERI integral.
#[allow(clippy::too_many_arguments, dead_code)]
pub fn compute_eri_primitive(
    alpha_p: f64,
    alpha_q: f64,
    alpha_r: f64,
    alpha_s: f64,
    A: Point3<f64>,
    B: Point3<f64>,
    C: Point3<f64>,
    D: Point3<f64>,
) -> f64 {
    let p = alpha_p + alpha_q;
    let q = alpha_r + alpha_s;
    let alpha = (p * q) / (p + q);

    // Calculate centers P and Q
    let P = (alpha_p * A.coords + alpha_q * B.coords) / p;
    let Q = (alpha_r * C.coords + alpha_s * D.coords) / q;

    // Calculate distances
    let AB_sq = (A.coords - B.coords).norm_squared();
    let CD_sq = (C.coords - D.coords).norm_squared();
    let PQ_sq = (P - Q).norm_squared();

    // Calculate exponential factors
    let K_ab = (-alpha_p * alpha_q * AB_sq / p).exp();
    let K_cd = (-alpha_r * alpha_s * CD_sq / q).exp();
    // Calculate the Boys function
    let T = alpha * PQ_sq;
    let F0 = crate::math_utils::boys_function(0, T);

    // Calculate the prefactor
    let prefactor = (2.0 * PI.powf(2.5)) / (p * q * (p + q).sqrt());

    prefactor * K_ab * K_cd * F0
}

///
/// # Arguments
///
/// * `basis` - Reference to the `Basis` object containing all basis functions.
///
/// # Returns
///
/// A 4D tensor containing all ERI integrals.
pub fn electron_repulsion_ints(basis: &Basis) -> Array4<f64> {
    let n = basis.nbasis();
    let n_pairs = n * (n + 1) / 2;
    let n_unique_quartets = n_pairs * (n_pairs + 1) / 2;
    let pair_expansions = build_pair_expansions(basis);
    let values = (0..n_unique_quartets)
        .into_par_iter()
        .map(|index| {
            let (pair_pq, pair_rs) = unique_pair_indices(index);
            let (p, q) = basis_function_pair(pair_pq);
            let (r, s) = basis_function_pair(pair_rs);
            (
                p,
                q,
                r,
                s,
                compute_eri_pair(&pair_expansions[pair_pq], &pair_expansions[pair_rs]),
            )
        })
        .collect::<Vec<_>>();

    let mut eri_tensor = Array4::zeros((n, n, n, n));
    for (p, q, r, s, value) in values {
        for (i, j, k, l) in eri_permutations(p, q, r, s) {
            eri_tensor[(i, j, k, l)] = value;
        }
    }
    eri_tensor
}

fn unique_pair_indices(index: usize) -> (usize, usize) {
    let pair_pq = (((8 * index + 1) as f64).sqrt() as usize - 1) / 2;
    let pair_rs = index - pair_pq * (pair_pq + 1) / 2;
    (pair_pq, pair_rs)
}

fn basis_function_pair(pair_index: usize) -> (usize, usize) {
    let first = (((8 * pair_index + 1) as f64).sqrt() as usize - 1) / 2;
    let second = pair_index - first * (first + 1) / 2;
    (first, second)
}

fn eri_permutations(p: usize, q: usize, r: usize, s: usize) -> [(usize, usize, usize, usize); 8] {
    [
        (p, q, r, s),
        (q, p, r, s),
        (p, q, s, r),
        (q, p, s, r),
        (r, s, p, q),
        (s, r, p, q),
        (r, s, q, p),
        (s, r, q, p),
    ]
}

struct PairPrimitive {
    exponent_sum: f64,
    center: Vector3<f64>,
    coefficient: f64,
    terms: Vec<HermiteTerm>,
    max_orders: Vector3<u8>,
}

struct HermiteTerm {
    orders: Vector3<u8>,
    coefficient: f64,
}

type PairExpansion = Vec<PairPrimitive>;

fn build_pair_expansions(basis: &Basis) -> Vec<PairExpansion> {
    let n = basis.nbasis();
    let n_pairs = n * (n + 1) / 2;
    (0..n_pairs)
        .into_par_iter()
        .map(|pair_index| {
            let (i, j) = basis_function_pair(pair_index);
            build_pair_expansion(basis, i, j)
        })
        .collect()
}

fn build_pair_expansion(basis: &Basis, i: usize, j: usize) -> PairExpansion {
    let shell_i = &basis.shells[basis.shell_ids[i]];
    let shell_j = &basis.shells[basis.shell_ids[j]];
    let origin_i = shell_i.origin;
    let origin_j = shell_j.origin;
    let mut expansion = Vec::new();

    for component_i in &basis.normalized_components[i] {
        for component_j in &basis.normalized_components[j] {
            for primitive_i in &component_i.primitives {
                for primitive_j in &component_j.primitives {
                    let exponent_sum = primitive_i.exponent + primitive_j.exponent;
                    let e = hermite_coefficients_3d(
                        component_i.angular_momentum,
                        component_j.angular_momentum,
                        origin_i - origin_j,
                        primitive_i.exponent,
                        primitive_j.exponent,
                    );
                    expansion.push(PairPrimitive {
                        exponent_sum,
                        center: gaussian_product_center(
                            primitive_i.exponent,
                            &origin_i,
                            primitive_j.exponent,
                            &origin_j,
                        ),
                        coefficient: primitive_i.coefficient * primitive_j.coefficient,
                        max_orders: Vector3::new(
                            e[0].len() as u8 - 1,
                            e[1].len() as u8 - 1,
                            e[2].len() as u8 - 1,
                        ),
                        terms: hermite_terms(&e),
                    });
                }
            }
        }
    }

    expansion
}

fn compute_eri_pair(pair_ab: &PairExpansion, pair_cd: &PairExpansion) -> f64 {
    let mut eri = 0.0;
    for primitive_ab in pair_ab {
        for primitive_cd in pair_cd {
            eri += primitive_ab.coefficient
                * primitive_cd.coefficient
                * compute_eri_pair_primitive(primitive_ab, primitive_cd);
        }
    }
    eri
}

fn compute_eri_pair_primitive(primitive_ab: &PairPrimitive, primitive_cd: &PairPrimitive) -> f64 {
    let p = primitive_ab.exponent_sum;
    let q = primitive_cd.exponent_sum;
    let alpha = p * q / (p + q);
    let pq = primitive_ab.center - primitive_cd.center;

    let max_orders = primitive_ab.max_orders + primitive_cd.max_orders;
    let mut coulomb_cache =
        CoulombAuxiliaryCache::new(max_orders.x, max_orders.y, max_orders.z, alpha, pq);

    let mut eri = 0.0;
    for term_ab in &primitive_ab.terms {
        for term_cd in &primitive_cd.terms {
            let orders = term_ab.orders + term_cd.orders;
            let sign = if term_cd.orders.sum() % 2 == 0 {
                1.0
            } else {
                -1.0
            };
            eri +=
                term_ab.coefficient * term_cd.coefficient * sign * coulomb_cache.value(orders, 0);
        }
    }

    2.0 * PI.powf(2.5) / (p * q * (p + q).sqrt()) * eri
}

fn hermite_terms(e: &[Vec<f64>; 3]) -> Vec<HermiteTerm> {
    let [e_x, e_y, e_z] = e;
    let mut terms = Vec::with_capacity(e_x.len() * e_y.len() * e_z.len());
    for (t, &x) in e_x.iter().enumerate() {
        for (u, &y) in e_y.iter().enumerate() {
            for (v, &z) in e_z.iter().enumerate() {
                let coefficients = Vector3::new(x, y, z);
                terms.push(HermiteTerm {
                    orders: Vector3::new(t as u8, u as u8, v as u8),
                    coefficient: coefficients.product(),
                });
            }
        }
    }
    terms
}

fn hermite_coefficients_3d(
    i_max: Vector3<u8>,
    j_max: Vector3<u8>,
    q: Vector3<f64>,
    a: f64,
    b: f64,
) -> [Vec<f64>; 3] {
    [
        hermite_coefficients(i_max.x, j_max.x, q.x, a, b),
        hermite_coefficients(i_max.y, j_max.y, q.y, a, b),
        hermite_coefficients(i_max.z, j_max.z, q.z, a, b),
    ]
}

fn hermite_coefficients(i_max: u8, j_max: u8, qx: f64, a: f64, b: f64) -> Vec<f64> {
    let t_max = i_max + j_max;
    let mut cache = HermiteCoefficientCache::new(i_max, j_max, t_max, qx, a, b);
    (0..=t_max).map(|t| cache.value(i_max, j_max, t)).collect()
}

struct HermiteCoefficientCache {
    j_len: usize,
    t_len: usize,
    qx: f64,
    a: f64,
    b: f64,
    values: Vec<Option<f64>>,
}

impl HermiteCoefficientCache {
    fn new(i_max: u8, j_max: u8, t_max: u8, qx: f64, a: f64, b: f64) -> Self {
        let len = (i_max as usize + 1) * (j_max as usize + 1) * (t_max as usize + 2);
        Self {
            j_len: j_max as usize + 1,
            t_len: t_max as usize + 2,
            qx,
            a,
            b,
            values: vec![None; len],
        }
    }

    fn value(&mut self, i: u8, j: u8, t: u8) -> f64 {
        if t > i + j {
            return 0.0;
        }
        let index = self.index(i, j, t);
        if let Some(value) = self.values[index] {
            return value;
        }

        let value = if i == 0 && j == 0 && t == 0 {
            let p = self.a + self.b;
            let reduced_exp = self.a * self.b / p;
            (-reduced_exp * self.qx.powi(2)).exp()
        } else if i == 0 && j == 0 {
            0.0
        } else {
            let p = self.a + self.b;
            let reduced_exp = self.a * self.b / p;
            if i > 0 {
                let lower_i = i - 1;
                let left = if t > 0 {
                    self.value(lower_i, j, t - 1) / (2.0 * p)
                } else {
                    0.0
                };
                let middle = -(reduced_exp * self.qx / self.a) * self.value(lower_i, j, t);
                let right = (t as f64 + 1.0) * self.value(lower_i, j, t + 1);
                left + middle + right
            } else {
                let lower_j = j - 1;
                let left = if t > 0 {
                    self.value(i, lower_j, t - 1) / (2.0 * p)
                } else {
                    0.0
                };
                let middle = (reduced_exp * self.qx / self.b) * self.value(i, lower_j, t);
                let right = (t as f64 + 1.0) * self.value(i, lower_j, t + 1);
                left + middle + right
            }
        };
        self.values[index] = Some(value);
        value
    }

    fn index(&self, i: u8, j: u8, t: u8) -> usize {
        ((i as usize * self.j_len) + j as usize) * self.t_len + t as usize
    }
}

struct CoulombAuxiliaryCache {
    u_len: usize,
    v_len: usize,
    n_len: usize,
    p: f64,
    pc: Vector3<f64>,
    values: Vec<Option<f64>>,
}

impl CoulombAuxiliaryCache {
    fn new(t_max: u8, u_max: u8, v_max: u8, p: f64, pc: Vector3<f64>) -> Self {
        let n_max = t_max + u_max + v_max;
        let len = (t_max as usize + 1)
            * (u_max as usize + 1)
            * (v_max as usize + 1)
            * (n_max as usize + 1);
        Self {
            u_len: u_max as usize + 1,
            v_len: v_max as usize + 1,
            n_len: n_max as usize + 1,
            p,
            pc,
            values: vec![None; len],
        }
    }

    fn value(&mut self, orders: Vector3<u8>, n: u8) -> f64 {
        self.value_at(orders.x, orders.y, orders.z, n)
    }

    fn value_at(&mut self, t: u8, u: u8, v: u8, n: u8) -> f64 {
        let index = self.index(t, u, v, n);
        if let Some(value) = self.values[index] {
            return value;
        }

        let value = if t == 0 && u == 0 && v == 0 {
            (-2.0 * self.p).powi(n as i32)
                * crate::math_utils::boys_function(n as u64, self.p * self.pc.norm_squared())
        } else if t > 0 {
            let lower = if t >= 2 {
                (t as f64 - 1.0) * self.value_at(t - 2, u, v, n + 1)
            } else {
                0.0
            };
            lower + self.pc.x * self.value_at(t - 1, u, v, n + 1)
        } else if u > 0 {
            let lower = if u >= 2 {
                (u as f64 - 1.0) * self.value_at(t, u - 2, v, n + 1)
            } else {
                0.0
            };
            lower + self.pc.y * self.value_at(t, u - 1, v, n + 1)
        } else {
            let lower = if v >= 2 {
                (v as f64 - 1.0) * self.value_at(t, u, v - 2, n + 1)
            } else {
                0.0
            };
            lower + self.pc.z * self.value_at(t, u, v - 1, n + 1)
        };
        self.values[index] = Some(value);
        value
    }

    fn index(&self, t: u8, u: u8, v: u8, n: u8) -> usize {
        (((t as usize * self.u_len) + u as usize) * self.v_len + v as usize) * self.n_len
            + n as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basis::gaussian::contraction::Contraction;
    use crate::molecules::atom::Atom;
    use crate::molecules::geometry::Geometry;
    use crate::test_utils;
    use approx::assert_abs_diff_eq;
    use nalgebra::point;

    /// Structure for a simple contraction (s-orbital, STO-3G).
    #[allow(dead_code)]
    fn create_sto3g_contraction() -> Contraction {
        let coefficients = vec![0.15432897, 0.53532814, 0.44463454];
        // Arguments: l = 0 (s-orbital), pure = false (Cartesian)
        Contraction::new(0, false, coefficients)
    }

    /// Helper function to create an H2 geometry.
    fn create_h2_geometry() -> Geometry {
        let elements = periodic_table::periodic_table();
        let h = &elements[0]; // Hydrogen
        let atom1 = Atom::new(h, point![0.0, 0.0, -1.40]);
        let atom2 = Atom::new(h, point![0.0, 0.0, 1.40]);
        Geometry::new("Hydrogen molecule (H2)".to_string(), vec![atom1, atom2])
    }

    /// Test ERI calculation for two s functions at the same center.
    #[test]
    fn test_compute_eri_s_same_center() {
        let alpha_p = 0.5;
        let alpha_q = 0.5;
        let alpha_r = 0.5;
        let alpha_s = 0.5;

        let A = Point3::new(0.0, 0.0, 0.0);
        let B = Point3::new(0.0, 0.0, 0.0);
        let C = Point3::new(0.0, 0.0, 0.0);
        let D = Point3::new(0.0, 0.0, 0.0);

        let eri = compute_eri_primitive(alpha_p, alpha_q, alpha_r, alpha_s, A, B, C, D);

        // Calculate the expected value
        let expected_eri = (2.0 * PI.powf(2.5))
            / (alpha_p + alpha_q)
            / (alpha_r + alpha_s)
            / (alpha_p + alpha_q + alpha_r + alpha_s).sqrt();

        assert_abs_diff_eq!(eri, expected_eri, epsilon = 1e-6);
    }

    /// Test ERI calculation for s functions with different centers.
    #[test]
    fn test_compute_eri_s_different_centers() {
        // Exponents
        let alpha_p = 0.5;
        let alpha_q = 0.5;
        let alpha_r = 0.5;
        let alpha_s = 0.5;

        // Center positions (in bohrs)
        let A = Point3::new(0.0, 0.0, 0.0);
        let B = Point3::new(0.0, 0.0, 1.0);
        let C = Point3::new(0.0, 0.0, 0.0);
        let D = Point3::new(0.0, 0.0, -1.0);

        let eri = compute_eri_primitive(alpha_p, alpha_q, alpha_r, alpha_s, A, B, C, D);

        let expected_eri = 12.838834347631737;

        // Check
        assert_abs_diff_eq!(eri, expected_eri, epsilon = 1e-6);
    }

    /// Test ERI calculation for the H2 hydrogen molecule.
    #[test]
    fn test_eri_hydrogen_molecule() {
        let basis_file = test_utils::load_minimal_basis_file();
        let geom = create_h2_geometry();
        let basis = Basis::load(&basis_file, &geom);

        // Calculate ERI integrals for the whole molecule
        let eri_tensor = electron_repulsion_ints(&basis);

        // Select the (0,1,0,1) integral for H2
        let eri = eri_tensor[(0, 1, 0, 1)];

        let expected_eri = 0.039595701902556416;

        // Check
        assert_abs_diff_eq!(eri, expected_eri, epsilon = 1e-6);
    }

    #[test]
    fn test_eri_hydrogen_molecule_self_integral() {
        let basis_file = test_utils::load_minimal_basis_file();
        let geom = create_h2_geometry();
        let basis = Basis::load(&basis_file, &geom);

        // Calculate ERI integrals for the whole molecule
        let eri_tensor = electron_repulsion_ints(&basis);

        // Select the (0,0,0,0) integral for H2
        let eri = eri_tensor[(0, 0, 0, 0)];

        // Approximate expected value for the self-integral (based on theory or other software)
        // This value should be obtained with reference software such as PySCF for better precision.
        let expected_eri_self = 0.7746059439198978;

        // Check
        assert_abs_diff_eq!(eri, expected_eri_self, epsilon = 1e-6);
    }

    /// Test the symmetry of ERI matrices.
    #[test]
    fn test_eri_symmetry() {
        let geom = create_h2_geometry();
        let basis_file = test_utils::load_minimal_basis_file();
        let basis = Basis::load(&basis_file, &geom);

        let eri_tensor = electron_repulsion_ints(&basis);

        for p in 0..basis.nbasis() {
            for q in 0..basis.nbasis() {
                for r in 0..basis.nbasis() {
                    for s in 0..basis.nbasis() {
                        assert_abs_diff_eq!(
                            eri_tensor[(p, q, r, s)],
                            eri_tensor[(q, p, r, s)],
                            epsilon = 1e-6
                        );
                        assert_abs_diff_eq!(
                            eri_tensor[(p, q, r, s)],
                            eri_tensor[(p, q, s, r)],
                            epsilon = 1e-6
                        );
                        assert_abs_diff_eq!(
                            eri_tensor[(p, q, r, s)],
                            eri_tensor[(r, s, p, q)],
                            epsilon = 1e-6
                        );
                        assert_abs_diff_eq!(
                            eri_tensor[(p, q, r, s)],
                            eri_tensor[(q, p, s, r)],
                            epsilon = 1e-6
                        );
                    }
                }
            }
        }
    }
}
