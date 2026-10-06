use crate::math_utils::f64_const::SQRT_PI;
use special::{Gamma, Primitive};
use std::ops::Index;

/// Calculate the Boys function $F_m(x)$ for a given order $m$ and parameter $x$.
///
/// $$
/// F_m(x) = \int_0^1 t^{2m} \exp(-x t^2)\,dt.
/// $$
///
/// The integral occurs in Gaussian Coulomb and electron-repulsion integrals.
#[allow(
    clippy::doc_markdown,
    reason = "MathJax renders the Boys function notation and integral as mathematical expressions"
)]
pub fn boys_function(m: u64, x: f64) -> f64 {
    boys_function_value(m, x)
}

#[derive(Debug, Clone)]
pub struct CachedBoysFunction {
    values: Vec<f64>,
}

impl CachedBoysFunction {
    pub fn new(max_order: u8, x: f64) -> Self {
        let count = max_order as usize + 1;

        let mut values = Vec::with_capacity(count);
        if x == 0.0 {
            for m in 0..=max_order {
                values.push(1.0 / f64::from(2 * u32::from(m) + 1));
            }
        } else if max_order == 0 {
            values.push(boys_zero(x));
        } else if x < 0.5 {
            for m in 0..=max_order {
                values.push(boys_function_value(u64::from(m), x));
            }
        } else {
            values.resize(count, 0.0);
            let exp_neg_x = (-x).exp();
            values[max_order as usize] = boys_gamma_reference(u64::from(max_order), x);
            for m in (0..max_order).rev() {
                values[m as usize] = (2.0 * x * values[m as usize + 1] + exp_neg_x)
                    / f64::from(2 * u32::from(m) + 1);
            }
        }

        Self { values }
    }
}

impl Index<u8> for CachedBoysFunction {
    type Output = f64;

    fn index(&self, order: u8) -> &Self::Output {
        &self.values[order as usize]
    }
}

fn boys_zero(x: f64) -> f64 {
    let sqrtx = x.sqrt();
    (SQRT_PI * Primitive::erf(sqrtx)) / (2.0 * sqrtx)
}

#[allow(
    clippy::cast_precision_loss,
    reason = "Scientific coefficients and analytic functions are evaluated at f64 precision"
)]
fn boys_function_value(m: u64, x: f64) -> f64 {
    if x == 0.0 {
        1.0 / (2 * m + 1) as f64
    } else if m == 0 {
        boys_zero(x)
    } else if x.abs() < (m as f64 + 0.5) * 1e-4 {
        boys_series(m, x)
    } else {
        boys_gamma_reference(m, x)
    }
}

#[allow(
    clippy::cast_precision_loss,
    reason = "Scientific coefficients and analytic functions are evaluated at f64 precision"
)]
fn boys_series(m: u64, x: f64) -> f64 {
    let mut term = 1.0 / (2 * m + 1) as f64;
    let mut sum = term;

    for k in 1..=100 {
        let k = f64::from(k);
        term *= -x / k * (2.0 * m as f64 + 2.0 * k - 1.0) / (2.0 * m as f64 + 2.0 * k + 1.0);
        sum += term;
        if term.abs() <= f64::EPSILON * sum.abs() {
            break;
        }
    }

    sum
}

#[allow(
    clippy::cast_precision_loss,
    reason = "Scientific coefficients and analytic functions are evaluated at f64 precision"
)]
fn boys_gamma_reference(m: u64, x: f64) -> f64 {
    let a = m as f64 + 0.5;
    let gamma_a = <f64 as Gamma>::gamma(a);
    let p_lower_gamma = <f64 as Gamma>::inc_gamma(x, a);
    let gamma_inc = p_lower_gamma * gamma_a;
    gamma_inc / (2.0 * x.powf(a))
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;
    use approx::assert_abs_diff_eq;

    #[allow(
        clippy::cast_precision_loss,
        reason = "Fixture dimensions and quadrature orders are small enough to be represented exactly in f64"
    )]
    fn boys_integral_reference(m: u64, x: f64) -> f64 {
        const INTERVALS: usize = 16_384;
        let h = 1.0 / INTERVALS as f64;
        let integrand = |t: f64| {
            t.powi(i32::try_from(2 * m).expect("quadrature test orders fit in i32"))
                * (-x * t * t).exp()
        };
        let interior = (1..INTERVALS)
            .map(|index| {
                let weight = if index % 2 == 0 { 2.0 } else { 4.0 };
                weight * integrand(index as f64 * h)
            })
            .sum::<f64>();
        h / 3.0 * (integrand(0.0) + interior + integrand(1.0))
    }

    #[test]
    fn test_sqrt_i() {
        assert_abs_diff_eq!(SQRT_PI, PI.sqrt(), epsilon = 1e-14);
    }

    #[test]
    fn test_boys_function() {
        // Reference values from reliable sources or verified calculations.
        let test_cases = [
            (0, 0.001, 0.999_666_766_642_861_8),
            (0, 0.5, 0.855_624),
            (3, 0.01, 0.141_750_564_407_793_24),
            (5, 0.015, 0.089_762_711_777_728_57),
            (0, 1.0, 0.746_824_132_812_427_1),
            (2, 5.0, 0.010_995_436_178_434_296),
            (4, 10.0, 0.000_180_619_436_364_399_07),
            (3, 7.5, 0.001_386_465_581_800_329_2),
            (0, 25.0, 0.177_245_385_090_279_07),
            (2, 50.0, 3.759_942_411_946_5e-05),
            (5, 100.0, 2.617_138_889_405_674_7e-10),
        ];
        for (m, x, expected) in test_cases {
            let result = boys_function(m, x);
            assert_abs_diff_eq!(result, expected, epsilon = 1e-6);
        }
    }

    #[test]
    #[allow(
        clippy::cast_precision_loss,
        reason = "Fixture dimensions and quadrature orders are small enough to be represented exactly in f64"
    )]
    fn optimized_boys_function_matches_exact_reference() {
        let x_values = [
            0.0, 1e-12, 1e-9, 1e-6, 1e-4, 1e-3, 0.01, 0.1, 0.2, 0.5, 1.0, 1.25, 1.5, 2.0, 3.0, 5.0,
            10.0, 25.0, 50.0, 100.0,
        ];

        for m in 0..=12 {
            for x in x_values {
                let result = boys_function(m, x);
                let expected = if x == 0.0 || (m > 0 && x.abs() < (m as f64 + 0.5) * 1e-4) {
                    boys_integral_reference(m, x)
                } else {
                    boys_gamma_reference(m, x)
                };
                let error = (result - expected).abs();
                assert!(
                    error <= 1e-11,
                    "m={m}, x={x}, result={result}, expected={expected}, error={error}"
                );
            }
        }
    }

    #[test]
    fn cached_boys_function_matches_independent_reference() {
        let x_values = [
            0.0, 1e-12, 1e-9, 1e-6, 1e-4, 1e-3, 0.01, 0.1, 0.2, 0.5, 1.0, 1.25, 1.5, 2.0, 3.0, 5.0,
            10.0, 25.0, 50.0, 100.0,
        ];

        for max_order in 0..=12 {
            for x in x_values {
                let cache = CachedBoysFunction::new(max_order, x);
                for m in 0..=max_order {
                    let result = cache[m];
                    let expected = if x == 0.0 || (m > 0 && x.abs() < (f64::from(m) + 0.5) * 1e-4) {
                        boys_integral_reference(u64::from(m), x)
                    } else {
                        boys_gamma_reference(u64::from(m), x)
                    };
                    let error = (result - expected).abs();
                    assert!(
                        error <= 1e-9,
                        "max_order={max_order}, m={m}, x={x}, result={result}, expected={expected}, error={error}"
                    );
                }
            }
        }
    }

    #[test]
    fn small_x_regressions_match_high_precision_values() {
        assert_abs_diff_eq!(
            boys_function(1, 1e-4),
            0.333_313_334_047_600,
            epsilon = 1e-15
        );
        assert_abs_diff_eq!(
            boys_function(12, 1e-3),
            0.039_962_980_198_967_2,
            epsilon = 1e-15
        );
    }
}
