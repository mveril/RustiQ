// src/eri.rs
#![allow(non_snake_case)]

use std::f64::consts::PI;

use crate::basis::gaussian::basis::{
    coulomb_auxiliary, gaussian_norm_const, gaussian_product_center, hermite_coeff, Basis,
};
use nalgebra::Point3;
use ndarray::Array4;
use rayon::prelude::*;

/// Calcule l'intégrale de recouvrement 1D pour deux fonctions gaussiennes primitives.
///
/// \[ S = \left( \frac{\pi}{p} \right)^{1/2} \exp\left( -\frac{\mu}{p} (A - B)^2 \right) \]
///
/// où \( p = \alpha_a + \alpha_b \) et \( \mu = \alpha_a \alpha_b \).
///
/// # Arguments
///
/// * `PAx` - Coordonnée x du vecteur PA.
/// * `PBx` - Coordonnée x du vecteur PB.
/// * `gamma` - \( p = \alpha_a + \alpha_b \).
///
/// # Retourne
///
/// L'intégrale de recouvrement 1D.
#[allow(dead_code)]
pub fn overlap_1d(PAx: f64, PBx: f64, gamma: f64) -> f64 {
    let T = gamma * (PAx - PBx).powi(2);
    (std::f64::consts::PI / gamma).sqrt() * (-T).exp()
}

/// Calcule l'intégrale cinétique 1D pour deux fonctions gaussiennes primitives.
///
/// \[ T = \frac{\alpha_a \alpha_b}{\gamma} (3) S \]
///
/// pour des orbitales s.
///
/// # Arguments
///
/// * `PAx` - Coordonnée x du vecteur PA.
/// * `PBx` - Coordonnée x du vecteur PB.
/// * `gamma` - \( \gamma = \alpha_a + \alpha_b \).
/// * `alpha_a` - Exposant gaussien de la première fonction.
/// * `alpha_b` - Exposant gaussien de la deuxième fonction.
///
/// # Retourne
///
/// L'intégrale cinétique 1D.
#[allow(dead_code)]
pub fn kinetic_1d(PAx: f64, PBx: f64, gamma: f64, alpha_a: f64, alpha_b: f64) -> f64 {
    let S = overlap_1d(PAx, PBx, gamma);
    (alpha_a * alpha_b / gamma) * 3.0 * S
}

/// Calcule l'intégrale ERI pour des primitives gaussiennes s.
///
/// \[ (ss|ss) = \frac{2 \pi^{5/2}}{p q \sqrt{p + q}} K_{ab} K_{cd} F_0(T) \]
///
/// # Arguments
///
/// * `alpha_p`, `alpha_q`, `alpha_r`, `alpha_s` - Exposants des primitives.
/// * `A` - Position du centre A.
/// * `B` - Position du centre B.
/// * `C` - Position du centre C.
/// * `D` - Position du centre D.
///
/// # Retourne
///
/// La valeur de l'intégrale ERI primitive.
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

    // Calcul des centres P et Q
    let P = (alpha_p * A.coords + alpha_q * B.coords) / p;
    let Q = (alpha_r * C.coords + alpha_s * D.coords) / q;

    // Calcul des distances
    let AB_sq = (A.coords - B.coords).norm_squared();
    let CD_sq = (C.coords - D.coords).norm_squared();
    let PQ_sq = (P - Q).norm_squared();

    // Calcul des facteurs exponentiels
    let K_ab = (-alpha_p * alpha_q * AB_sq / p).exp();
    let K_cd = (-alpha_r * alpha_s * CD_sq / q).exp();
    // Calcul de la fonction de Boys
    let T = alpha * PQ_sq;
    let F0 = crate::math_utils::boys_function(0, T);

    // Calcul du préfacteur
    let prefactor = (2.0 * PI.powf(2.5)) / (p * q * (p + q).sqrt());

    prefactor * K_ab * K_cd * F0
}

///
/// # Arguments
///
/// * `basis` - Référence à l'objet `Basis` contenant toutes les fonctions de base.
///
/// # Retourne
///
/// Un tenseur 4D contenant toutes les intégrales ERI.
pub fn electron_repulsion_ints(basis: &Basis) -> Array4<f64> {
    let n = basis.nbasis();
    let values = (0..n * n * n * n)
        .into_par_iter()
        .map(|index| {
            let s = index % n;
            let r = (index / n) % n;
            let q = (index / (n * n)) % n;
            let p = index / (n * n * n);
            let shell_p = &basis.shells[basis.shell_ids[p]];
            let origin_p = shell_p.origin;

            let shell_q = &basis.shells[basis.shell_ids[q]];
            let origin_q = shell_q.origin;

            let shell_r = &basis.shells[basis.shell_ids[r]];
            let origin_r = shell_r.origin;

            let shell_s = &basis.shells[basis.shell_ids[s]];
            let origin_s = shell_s.origin;

            let mut eri_pqrs = 0.0;

            // Boucles sur les contractions de chaque shell
            for contraction_p in &shell_p.contr {
                for contraction_q in &shell_q.contr {
                    for contraction_r in &shell_r.contr {
                        for contraction_s in &shell_s.contr {
                            // Boucles sur les primitives de chaque contraction
                            for (&alpha_p, &coeff_p) in
                                shell_p.alpha.iter().zip(contraction_p.coeff.iter())
                            {
                                for (&alpha_q, &coeff_q) in
                                    shell_q.alpha.iter().zip(contraction_q.coeff.iter())
                                {
                                    for (&alpha_r, &coeff_r) in
                                        shell_r.alpha.iter().zip(contraction_r.coeff.iter())
                                    {
                                        for (&alpha_s, &coeff_s) in
                                            shell_s.alpha.iter().zip(contraction_s.coeff.iter())
                                        {
                                            // Calcul des constantes de normalisation
                                            let l_p = basis.angular_momenta[p];
                                            let l_q = basis.angular_momenta[q];
                                            let l_r = basis.angular_momenta[r];
                                            let l_s = basis.angular_momenta[s];
                                            let norm_p = gaussian_norm_const(
                                                alpha_p,
                                                l_p.x as u32,
                                                l_p.y as u32,
                                                l_p.z as u32,
                                            );
                                            let norm_q = gaussian_norm_const(
                                                alpha_q,
                                                l_q.x as u32,
                                                l_q.y as u32,
                                                l_q.z as u32,
                                            );
                                            let norm_r = gaussian_norm_const(
                                                alpha_r,
                                                l_r.x as u32,
                                                l_r.y as u32,
                                                l_r.z as u32,
                                            );
                                            let norm_s = gaussian_norm_const(
                                                alpha_s,
                                                l_s.x as u32,
                                                l_s.y as u32,
                                                l_s.z as u32,
                                            );

                                            // Calcul des positions intermédiaires
                                            let A = origin_p;
                                            let B = origin_q;
                                            let C = origin_r;
                                            let D = origin_s;
                                            let eri = compute_eri_cartesian_primitive(
                                                alpha_p, alpha_q, alpha_r, alpha_s, A, B, C, D,
                                                &l_p, &l_q, &l_r, &l_s,
                                            );

                                            // Contribution à l'ERI total
                                            eri_pqrs += coeff_p
                                                * coeff_q
                                                * coeff_r
                                                * coeff_s
                                                * norm_p
                                                * norm_q
                                                * norm_r
                                                * norm_s
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
        })
        .collect::<Vec<_>>();

    Array4::from_shape_vec((n, n, n, n), values)
        .expect("ERI tensor shape should match computed value count")
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
    use crate::molecules::units::Units;
    use crate::test_utils;
    use approx::assert_abs_diff_eq;
    use nalgebra::point;

    /// Structure pour une contraction simple (s-orbital, STO-3G).
    #[allow(dead_code)]
    fn create_sto3g_contraction() -> Contraction {
        let coefficients = vec![0.15432897, 0.53532814, 0.44463454];
        // Arguments : l = 0 (s-orbital), pure = false (cartésien)
        Contraction::new(0, false, coefficients)
    }

    /// Fonction utilitaire pour créer une géométrie H2.
    fn create_h2_geometry() -> Geometry {
        let elements = periodic_table::periodic_table();
        let h = &elements[0]; // Hydrogène
        let atom1 = Atom::new(h, point![0.0, 0.0, -1.40]);
        let atom2 = Atom::new(h, point![0.0, 0.0, 1.40]);
        Geometry::new(
            "Hydrogen molecule (H2)".to_string(),
            vec![atom1, atom2],
            Some(Units::Bohr),
            Some(Units::Bohr),
        )
    }

    /// Test de calcul de l'ERI pour deux fonctions s au même centre.
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

        // Calcul de la valeur attendue
        let expected_eri = (2.0 * PI.powf(2.5))
            / (alpha_p + alpha_q)
            / (alpha_r + alpha_s)
            / (alpha_p + alpha_q + alpha_r + alpha_s).sqrt();

        assert_abs_diff_eq!(eri, expected_eri, epsilon = 1e-6);
    }

    /// Test de calcul de l'ERI pour des fonctions s avec des centres différents.
    #[test]
    fn test_compute_eri_s_different_centers() {
        // Exposants
        let alpha_p = 0.5;
        let alpha_q = 0.5;
        let alpha_r = 0.5;
        let alpha_s = 0.5;

        // Positions des centres (en bohrs)
        let A = Point3::new(0.0, 0.0, 0.0);
        let B = Point3::new(0.0, 0.0, 1.0);
        let C = Point3::new(0.0, 0.0, 0.0);
        let D = Point3::new(0.0, 0.0, -1.0);

        let eri = compute_eri_primitive(alpha_p, alpha_q, alpha_r, alpha_s, A, B, C, D);

        let expected_eri = 12.838834347631737;

        // Vérification
        assert_abs_diff_eq!(eri, expected_eri, epsilon = 1e-6);
    }

    /// Test de calcul de l'ERI pour la molécule d'hydrogène H2.
    #[test]
    fn test_eri_hydrogen_molecule() {
        let basis_file = test_utils::load_minimal_basis_file();
        let geom = create_h2_geometry();
        let basis = Basis::load(&basis_file, &geom);

        // Calcul des intégrales ERI pour toute la molécule
        let eri_tensor = electron_repulsion_ints(&basis);

        // Sélection de l'intégrale (0,1,0,1) pour H2
        let eri = eri_tensor[(0, 1, 0, 1)];

        let expected_eri = 0.039595701902556416;

        // Vérification
        assert_abs_diff_eq!(eri, expected_eri, epsilon = 1e-6);
    }

    #[test]
    fn test_eri_hydrogen_molecule_self_integral() {
        let basis_file = test_utils::load_minimal_basis_file();
        let geom = create_h2_geometry();
        let basis = Basis::load(&basis_file, &geom);

        // Calcul des intégrales ERI pour toute la molécule
        let eri_tensor = electron_repulsion_ints(&basis);

        // Sélection de l'intégrale (0,0,0,0) pour H2
        let eri = eri_tensor[(0, 0, 0, 0)];

        // Valeur attendue approximative pour l'intégrale auto-cohrente (basée sur la théorie ou d'autres logiciels)
        // Vous devez obtenir cette valeur avec un logiciel de référence comme PySCF pour plus de précision.
        let expected_eri_self = 0.7746059439198978;

        // Vérification
        assert_abs_diff_eq!(eri, expected_eri_self, epsilon = 1e-6);
    }

    /// Test de la symétrie des matrices d'ERI.
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
