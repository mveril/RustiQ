// src/eri.rs
#![allow(non_snake_case)]

use std::f64::consts::PI;

use crate::basis::gaussian::basis::{
    coulomb_auxiliary, gaussian_product_center, hermite_coeff, Basis,
};
use nalgebra::Point3;
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
    (std::f64::consts::PI / gamma).sqrt() * (-T).exp()
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
    let values = (0..n_unique_quartets)
        .into_par_iter()
        .map(|index| {
            let (pair_pq, pair_rs) = unique_pair_indices(index);
            let (p, q) = basis_function_pair(pair_pq);
            let (r, s) = basis_function_pair(pair_rs);
            (p, q, r, s, compute_eri_ao(basis, p, q, r, s))
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

#[allow(clippy::too_many_lines)]
fn compute_eri_ao(basis: &Basis, p: usize, q: usize, r: usize, s: usize) -> f64 {
    let origin_p = basis.shells[basis.shell_ids[p]].origin;
    let origin_q = basis.shells[basis.shell_ids[q]].origin;
    let origin_r = basis.shells[basis.shell_ids[r]].origin;
    let origin_s = basis.shells[basis.shell_ids[s]].origin;

    let mut eri_pqrs = 0.0;
    for component_p in &basis.normalized_components[p] {
        for component_q in &basis.normalized_components[q] {
            for component_r in &basis.normalized_components[r] {
                for component_s in &basis.normalized_components[s] {
                    for primitive_p in &component_p.primitives {
                        for primitive_q in &component_q.primitives {
                            for primitive_r in &component_r.primitives {
                                for primitive_s in &component_s.primitives {
                                    let eri = compute_eri_cartesian_primitive(
                                        primitive_p.exponent,
                                        primitive_q.exponent,
                                        primitive_r.exponent,
                                        primitive_s.exponent,
                                        origin_p,
                                        origin_q,
                                        origin_r,
                                        origin_s,
                                        &component_p.angular_momentum,
                                        &component_q.angular_momentum,
                                        &component_r.angular_momentum,
                                        &component_s.angular_momentum,
                                    );
                                    eri_pqrs += primitive_p.coefficient
                                        * primitive_q.coefficient
                                        * primitive_r.coefficient
                                        * primitive_s.coefficient
                                        * eri;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    eri_pqrs
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

#[allow(clippy::too_many_arguments)]
fn compute_eri_cartesian_primitive(
    alpha_p: f64,
    alpha_q: f64,
    alpha_r: f64,
    alpha_s: f64,
    a: Point3<f64>,
    b: Point3<f64>,
    c: Point3<f64>,
    d: Point3<f64>,
    l_p: &nalgebra::Vector3<u8>,
    l_q: &nalgebra::Vector3<u8>,
    l_r: &nalgebra::Vector3<u8>,
    l_s: &nalgebra::Vector3<u8>,
) -> f64 {
    let p = alpha_p + alpha_q;
    let q = alpha_r + alpha_s;
    let alpha = p * q / (p + q);
    let p_center = gaussian_product_center(alpha_p, &a, alpha_q, &b);
    let q_center = gaussian_product_center(alpha_r, &c, alpha_s, &d);
    let pq = p_center - q_center;

    let e_ab_x = (0..=l_p.x + l_q.x)
        .map(|t| hermite_coeff(l_p.x, l_q.x, t, a.x - b.x, alpha_p, alpha_q))
        .collect::<Vec<_>>();
    let e_ab_y = (0..=l_p.y + l_q.y)
        .map(|u| hermite_coeff(l_p.y, l_q.y, u, a.y - b.y, alpha_p, alpha_q))
        .collect::<Vec<_>>();
    let e_ab_z = (0..=l_p.z + l_q.z)
        .map(|v| hermite_coeff(l_p.z, l_q.z, v, a.z - b.z, alpha_p, alpha_q))
        .collect::<Vec<_>>();
    let e_cd_x = (0..=l_r.x + l_s.x)
        .map(|t| hermite_coeff(l_r.x, l_s.x, t, c.x - d.x, alpha_r, alpha_s))
        .collect::<Vec<_>>();
    let e_cd_y = (0..=l_r.y + l_s.y)
        .map(|u| hermite_coeff(l_r.y, l_s.y, u, c.y - d.y, alpha_r, alpha_s))
        .collect::<Vec<_>>();
    let e_cd_z = (0..=l_r.z + l_s.z)
        .map(|v| hermite_coeff(l_r.z, l_s.z, v, c.z - d.z, alpha_r, alpha_s))
        .collect::<Vec<_>>();

    let mut eri = 0.0;
    for t in 0..=l_p.x + l_q.x {
        for u in 0..=l_p.y + l_q.y {
            for v in 0..=l_p.z + l_q.z {
                for tau in 0..=l_r.x + l_s.x {
                    for nu in 0..=l_r.y + l_s.y {
                        for phi in 0..=l_r.z + l_s.z {
                            let sign = if (tau + nu + phi) % 2 == 0 { 1.0 } else { -1.0 };
                            eri += e_ab_x[t as usize]
                                * e_ab_y[u as usize]
                                * e_ab_z[v as usize]
                                * e_cd_x[tau as usize]
                                * e_cd_y[nu as usize]
                                * e_cd_z[phi as usize]
                                * sign
                                * coulomb_auxiliary(t + tau, u + nu, v + phi, 0, alpha, &pq);
                        }
                    }
                }
            }
        }
    }

    2.0 * PI.powf(2.5) / (p * q * (p + q).sqrt()) * eri
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
