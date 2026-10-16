use super::contraction::Contraction;
use nalgebra::{DVector, Point3};
use rayon::iter::{ParallelBridge, ParallelIterator};
use std::f64::consts::PI;
use std::sync::LazyLock;

#[allow(
    dead_code,
    reason = "Retained scientific helpers and representations support tests, benchmarks, or future internal use"
)]
static SQRT_PI_CUBED: LazyLock<f64> = LazyLock::new(|| PI.powi(3).sqrt());

#[derive(Debug)]
pub(crate) struct InvalidContractionNorm;

#[derive(PartialEq, Debug, Clone)]
pub struct Shell {
    pub alpha: DVector<f64>,        // Gaussian exponents
    pub contr: Vec<Contraction>,    // Associated contractions
    pub origin: Point3<f64>,        // Atom position
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

    pub(crate) fn try_new(
        alpha: Vec<f64>,
        mut contr: Vec<Contraction>,
        origin: Point3<f64>,
    ) -> Result<Self, InvalidContractionNorm> {
        let alpha = DVector::<f64>::from_vec(alpha);
        let max_ln_coeff = Self::try_renorm(&alpha, &mut contr)?;
        Ok(Self {
            alpha,
            contr,
            origin,
            max_ln_coeff,
        })
    }

    #[allow(
        dead_code,
        reason = "Retained scientific helpers and representations support tests, benchmarks, or future internal use"
    )]
    pub fn nprim(&self) -> usize {
        self.alpha.len()
    }

    /// Normalizes the contraction coefficients
    pub fn renorm(alpha: &DVector<f64>, contr: &mut [Contraction]) -> DVector<f64> {
        for c in contr.iter() {
            assert_eq!(
                alpha.len(),
                c.coeff.len(),
                "Mismatch between alpha.len() and c.coeff.len() for contraction with l = {}",
                c.l
            );
            assert!(
                !alpha.is_empty(),
                "The exponent vector is empty for a contraction with l = {}",
                c.l
            );
        }
        Self::try_renorm(alpha, contr).expect("contraction must have a finite positive norm")
    }

    fn try_renorm(
        alpha: &DVector<f64>,
        contr: &mut [Contraction],
    ) -> Result<DVector<f64>, InvalidContractionNorm> {
        for c in contr.iter_mut() {
            if c.l > 15
                || alpha.len() != c.coeff.len()
                || alpha.is_empty()
                || alpha
                    .iter()
                    .any(|value| !value.is_finite() || *value <= 0.0)
                || c.coeff.iter().any(|value| !value.is_finite())
            {
                return Err(InvalidContractionNorm);
            }

            let coefficient_scale = c.coeff.iter().map(|value| value.abs()).fold(0.0, f64::max);
            if !coefficient_scale.is_finite() || coefficient_scale == 0.0 {
                return Err(InvalidContractionNorm);
            }
            // The overall coefficient scale cancels out of normalization. Remove
            // it before squaring coefficients, including subnormal input values.
            c.coeff /= coefficient_scale;
            let norm = Self::compute_contraction_norm(alpha, c);
            if !norm.is_finite() || norm <= 0.0 {
                return Err(InvalidContractionNorm);
            }
            c.coeff /= norm.sqrt();

            let norm_after = Self::compute_contraction_norm(alpha, c);
            if !norm_after.is_finite() || (norm_after - 1.0).abs() >= 1e-6 {
                return Err(InvalidContractionNorm);
            }
        }

        let max_ln_coeff = Self::update_max_ln_coeff(alpha, contr);
        if contr
            .iter()
            .flat_map(|c| c.coeff.iter())
            .any(|value| !value.is_finite())
        {
            return Err(InvalidContractionNorm);
        }
        Ok(max_ln_coeff)
    }

    fn compute_contraction_norm(alpha: &DVector<f64>, c: &Contraction) -> f64 {
        let mut norm = 0.0;
        let mut compensation = 0.0;
        let np = alpha.len();
        for p in 0..np {
            for q in 0..=p {
                let a = if p == q { 1.0 } else { 2.0 };
                // The normalized primitive overlap is (2√(αβ)/(α+β))^(l+3/2).
                // Set r=√min(α,β)/√max(α,β), giving the ratio 2r/(1+r²).
                // Separate square roots avoid underflow before taking the root;
                // neither an exponent product nor an exponent sum is needed.
                let smaller = alpha[p].min(alpha[q]);
                let larger = alpha[p].max(alpha[q]);
                let root_ratio = smaller.sqrt() / larger.sqrt();
                let overlap_ratio = 2.0 * root_ratio / (1.0 + root_ratio * root_ratio);
                let overlap = overlap_ratio.powf(f64::from(c.l) + 1.5);
                let term = a * overlap * c.coeff[p] * c.coeff[q];
                let corrected = term - compensation;
                let updated = norm + corrected;
                compensation = (updated - norm) - corrected;
                norm = updated;
            }
        }
        norm
    }

    pub fn update_max_ln_coeff(alpha: &DVector<f64>, contr: &[Contraction]) -> DVector<f64> {
        let mut ret = DVector::from_element(alpha.len(), f64::NEG_INFINITY);

        for c in contr {
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

    #[allow(
        dead_code,
        reason = "Retained scientific helpers and representations support tests, benchmarks, or future internal use"
    )]
    pub fn cartesian_size(&self) -> usize {
        self.contr
            .iter()
            .map(super::contraction::Contraction::cartesian_size)
            .sum()
    }

    #[allow(
        dead_code,
        reason = "Retained scientific helpers and representations support tests, benchmarks, or future internal use"
    )]
    pub fn size(&self) -> usize {
        self.contr
            .iter()
            .map(super::contraction::Contraction::size)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn generated_coefficient_scales_preserve_normalized_functions(
            l in 0u8..=15,
            log_scale in -300.0f64..300.0,
            coefficients in (0.8f64..1.2, -0.4f64..0.4, -0.4f64..0.4),
        ) {
            let coefficients = vec![coefficients.0, coefficients.1, coefficients.2];
            let reference = Shell::try_new(
                vec![0.3, 1.2, 4.0],
                vec![Contraction::new(l, false, coefficients.clone())],
                Point3::origin(),
            ).unwrap();
            let scale = 10.0_f64.powf(log_scale);
            let scaled = Shell::try_new(
                vec![0.3, 1.2, 4.0],
                vec![Contraction::new(l, false,
                    coefficients.iter().map(|value| value * scale).collect())],
                Point3::origin(),
            ).unwrap();
            prop_assert!((&scaled.contr[0].coeff - &reference.contr[0].coeff).amax() < 1e-13);
        }
    }

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
            "Normalization failed: norm = {norm}"
        );
    }

    #[test]
    fn high_angular_momentum_contraction_avoids_normalizer_overflow() {
        let shell = Shell::new(
            vec![1.0e20],
            vec![Contraction::new(15, false, vec![1.0])],
            Point3::origin(),
        );

        assert_eq!(shell.contr[0].coeff[0], 1.0);
    }

    #[test]
    fn rejects_zero_function_from_opposite_equal_primitives() {
        let error = Shell::try_new(
            vec![0.7, 0.7],
            vec![Contraction::new(0, false, vec![1.0, -1.0])],
            Point3::origin(),
        );
        assert!(error.is_err());
    }

    #[test]
    fn coefficient_rescaling_does_not_change_normalized_contraction() {
        let alpha = vec![0.2, 0.9, 3.1];
        let coefficients = [0.6, -0.25, 0.8];
        let mut reference = vec![Contraction::new(2, false, coefficients.to_vec())];
        Shell::renorm(&alpha.clone().into(), &mut reference);

        for scale in [1.0e-300, 1.0e300] {
            let mut scaled = vec![Contraction::new(
                2,
                false,
                coefficients.iter().map(|value| value * scale).collect(),
            )];
            Shell::renorm(&alpha.clone().into(), &mut scaled);
            for (actual, expected) in scaled[0].coeff.iter().zip(reference[0].coeff.iter()) {
                assert!((actual - expected).abs() < 1.0e-14);
            }
        }
    }

    #[test]
    fn normalized_overlap_handles_extreme_finite_exponents() {
        for exponent in [f64::from_bits(1), 1.0e-300, 1.0e300, f64::MAX] {
            let shell = Shell::try_new(
                vec![exponent],
                vec![Contraction::new(0, false, vec![1.0])],
                Point3::origin(),
            )
            .unwrap();
            assert!(shell.contr[0].coeff[0].is_finite());
            assert_eq!(shell.contr[0].coeff[0], 1.0);
        }
    }

    #[test]
    fn normalization_is_invariant_under_extreme_exponent_scaling() {
        for l in 0..=15 {
            let reference = Shell::try_new(
                vec![0.5, 1.5],
                vec![Contraction::new(l, false, vec![1.0, -0.25])],
                Point3::origin(),
            )
            .unwrap();
            for scale in [1.0e-300, 1.0e300] {
                let scaled = Shell::try_new(
                    vec![0.5 * scale, 1.5 * scale],
                    vec![Contraction::new(l, false, vec![1.0, -0.25])],
                    Point3::origin(),
                )
                .unwrap();
                assert!((&scaled.contr[0].coeff - &reference.contr[0].coeff).amax() < 1e-14);
            }
        }
    }

    #[test]
    fn rejects_non_finite_normalization_inputs() {
        for exponent in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
            assert!(Shell::try_new(
                vec![exponent],
                vec![Contraction::new(1, false, vec![1.0])],
                Point3::origin(),
            )
            .is_err());
        }
        for coefficient in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(Shell::try_new(
                vec![1.0],
                vec![Contraction::new(1, false, vec![coefficient])],
                Point3::origin(),
            )
            .is_err());
        }
    }

    #[test]
    fn test_load_6_31g_hydrogen() {
        let alpha_values = vec![6.362_421_39, 1.168_641_08, 0.380_389_00];
        let coeffs = vec![0.154_328_97, 0.535_328_14, 0.444_634_54];
        let l = 0;
        let pure = false;

        let alpha = DVector::from_vec(alpha_values);

        let mut contractions = [Contraction::new(l, pure, coeffs)];
        Shell::renorm(&alpha, &mut contractions);
        let norm = Shell::compute_contraction_norm(&alpha, &contractions[0]);

        assert!(
            (norm - 1.0).abs() < 1e-6,
            "Normalization failed for 6-31G hydrogen 1s orbital: norm = {norm}"
        );
    }

    #[test]
    #[should_panic(expected = "Mismatch between alpha.len() and c.coeff.len()")]
    fn test_contraction_with_empty_coefficients() {
        let alpha_values = vec![0.5, 1.0];
        let coeffs = vec![]; // Empty coefficients
        let l = 0;
        let pure = false;

        let alpha = DVector::from_vec(alpha_values);

        let mut contractions = [Contraction::new(l, pure, coeffs)];
        Shell::renorm(&alpha, &mut contractions);
    }

    #[test]
    #[should_panic(expected = "The exponent vector is empty")]
    fn test_contraction_with_empty_alpha() {
        let alpha_values = vec![]; // Empty exponents
        let coeffs = vec![]; // Matching
        let l = 0;
        let pure = false;

        let alpha = DVector::from_vec(alpha_values);

        let mut contractions = [Contraction::new(l, pure, coeffs)];
        Shell::renorm(&alpha, &mut contractions);
    }
}
