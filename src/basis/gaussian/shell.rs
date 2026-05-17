use super::contraction::Contraction;
use factorial::DoubleFactorial;
use nalgebra::{DVector, Point3};
use rayon::iter::{ParallelBridge, ParallelIterator};
use std::f64::consts::PI;
use std::sync::LazyLock;

#[allow(dead_code)]
static SQRT_PI_CUBED: LazyLock<f64> = LazyLock::new(|| PI.powi(3).sqrt());

#[derive(PartialEq, Debug, Clone)]
pub struct Shell {
    pub alpha: DVector<f64>,        // Exposants gaussiens
    pub contr: Vec<Contraction>,    // Contractions associées
    pub origin: Point3<f64>,        // Position de l'atome
    pub max_ln_coeff: DVector<f64>, // Max ln(coeff)
}

impl Shell {
    pub fn new(alpha: Vec<f64>, mut contr: Vec<Contraction>, origin: Point3<f64>) -> Self {
        let alpha = DVector::<f64>::from_vec(alpha);
        let max_ln_coeff = Self::renorm(&alpha, &mut contr);
        Self {
            alpha,
            contr,
            origin,
            max_ln_coeff,
        }
    }

    #[allow(dead_code)]
    pub fn nprim(&self) -> usize {
        self.alpha.len()
    }

    /// Normalise les coefficients de la contraction
    pub fn renorm(alpha: &DVector<f64>, contr: &mut [Contraction]) -> DVector<f64> {
        for c in contr.iter_mut() {
            assert!(c.l <= 15, "Le moment angulaire l doit être <= 15");
            assert_eq!(
                alpha.len(),
                c.coeff.len(),
                "Mismatch between alpha.len() and c.coeff.len() for contraction with l = {}",
                c.l
            );

            if alpha.is_empty() {
                panic!(
                    "Le vecteur alpha est vide pour une contraction avec l = {}",
                    c.l
                );
            }

            let norm = Self::compute_contraction_norm(alpha, c);
            c.coeff /= norm.sqrt();

            let norm_after = Self::compute_contraction_norm(alpha, c);
            assert!(
                (norm_after - 1.0).abs() < 1e-6,
                "Norme de la contraction non normalisée: {}",
                norm_after
            );
        }

        Self::update_max_ln_coeff(alpha, contr)
    }

    fn compute_contraction_norm(alpha: &DVector<f64>, c: &Contraction) -> f64 {
        let mut norm = 0.0;
        let np = alpha.len();
        for p in 0..np {
            for q in 0..=p {
                let gamma = alpha[p] + alpha[q];
                let a = if p == q { 1.0 } else { 2.0 };

                let n_p = (2.0 * alpha[p] / PI).powf(0.75) * (4.0 * alpha[p]).powi(c.l as i32)
                    / ((2 * c.l as u64 + 1).double_factorial() as f64).sqrt();
                let n_q = (2.0 * alpha[q] / PI).powf(0.75) * (4.0 * alpha[q]).powi(c.l as i32)
                    / ((2 * c.l as u64 + 1).double_factorial() as f64).sqrt();

                let prefactor = n_p * n_q * (PI / gamma).powf(1.5);
                let exponent = (4.0 * alpha[p] * alpha[q] / gamma.powi(2)).powi(c.l as i32);
                let df_l = if c.l > 0 {
                    (2 * c.l as u64 - 1).double_factorial() as f64
                } else {
                    1.0
                };
                let s_ij = prefactor * exponent * df_l;

                norm += a * s_ij * c.coeff[p] * c.coeff[q];
            }
        }
        norm
    }

    pub fn update_max_ln_coeff(alpha: &DVector<f64>, contr: &[Contraction]) -> DVector<f64> {
        let mut ret = DVector::from_element(alpha.len(), f64::NEG_INFINITY);

        for c in contr.iter() {
            let ln_coeffs = c.coeff.map(|coeff| {
                if coeff.abs() > 0.0 {
                    coeff.abs().ln()
                } else {
                    f64::NEG_INFINITY
                }
            });

            ret.as_mut_slice()
                .iter_mut()
                .zip(ln_coeffs.as_slice().iter())
                .par_bridge()
                .for_each(|(ret_elem, &ln_coeff)| {
                    *ret_elem = ret_elem.max(ln_coeff);
                });
        }

        ret
    }

    #[allow(dead_code)]
    pub fn cartesian_size(&self) -> usize {
        self.contr.iter().map(|c| c.cartesian_size()).sum()
    }

    #[allow(dead_code)]
    pub fn size(&self) -> usize {
        self.contr.iter().map(|c| c.size()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_renorm() {
        let alpha = vec![0.5, 1.0].into();
        let mut contr = vec![Contraction {
            l: 0,
            pure: false,
            coeff: vec![1.0, 1.0].into(),
        }];
        let _ = Shell::renorm(&alpha, &mut contr);

        let norm = Shell::compute_contraction_norm(&alpha, &contr[0]);
        assert!(
            (norm - 1.0).abs() < 1e-6,
            "Normalization failed: norm = {}",
            norm
        );
    }

    #[test]
    fn test_load_6_31g_hydrogen() {
        let alpha_values = vec![6.36242139, 1.16864108, 0.38038900];
        let coeffs = vec![0.15432897, 0.53532814, 0.44463454];
        let l = 0;
        let pure = false;

        let alpha = DVector::from_vec(alpha_values.clone());

        let mut contractions = [Contraction::new(l, pure, coeffs)];
        Shell::renorm(&alpha, &mut contractions);
        let norm = Shell::compute_contraction_norm(&alpha, &contractions[0]);

        assert!(
            (norm - 1.0).abs() < 1e-6,
            "Normalization failed for 6-31G hydrogen 1s orbital: norm = {}",
            norm
        );
    }

    #[test]
    #[should_panic(expected = "Mismatch between alpha.len() and c.coeff.len()")]
    fn test_contraction_with_empty_coefficients() {
        let alpha_values = vec![0.5, 1.0];
        let coeffs = vec![]; // Coefficients vides
        let l = 0;
        let pure = false;

        let alpha = DVector::from_vec(alpha_values.clone());

        let mut contractions = [Contraction::new(l, pure, coeffs)];
        Shell::renorm(&alpha, &mut contractions);
    }

    #[test]
    #[should_panic(expected = "Le vecteur alpha est vide")]
    fn test_contraction_with_empty_alpha() {
        let alpha_values = vec![]; // Exponents vides
        let coeffs = vec![]; // Correspondant
        let l = 0;
        let pure = false;

        let alpha = DVector::from_vec(alpha_values.clone());

        let mut contractions = [Contraction::new(l, pure, coeffs)];
        Shell::renorm(&alpha, &mut contractions);
    }
}
