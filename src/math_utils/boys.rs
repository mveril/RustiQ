use crate::math_utils::F64Const::SQRT_PI;
use special::{Gamma, Primitive};
use std::ops::Index;

/// Calculate the Boys function F_m(x) for a given order m and parameter x.
pub fn boys_function(m: u64, x: f64) -> f64 {
    boys_function_compatible(m, x)
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
                values.push(1.0 / (2 * m as u64 + 1) as f64);
            }
        } else if max_order == 0 {
            values.push(boys_zero(x));
        } else if x < 0.5 {
            for m in 0..=max_order {
                values.push(boys_function_compatible(m as u64, x));
            }
        } else {
            for _ in 0..=max_order {
                values.push(0.0);
            }
            let exp_neg_x = (-x).exp();
            values[max_order as usize] = boys_gamma_reference(max_order as u64, x);
            for m in (0..max_order).rev() {
                values[m as usize] =
                    (2.0 * x * values[m as usize + 1] + exp_neg_x) / (2 * m as u64 + 1) as f64;
            }
            for m in 1..=max_order {
                if x.abs() < (m as f64 + 0.5) * 1e-4 {
                    values[m as usize] = 1.0 / (2.0 * m as f64 + 1.0);
                }
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

fn boys_function_compatible(m: u64, x: f64) -> f64 {
    if x == 0.0 {
        1.0 / (2 * m + 1) as f64
    } else if m == 0 {
        boys_zero(x)
    } else if x.abs() < (m as f64 + 0.5) * 1e-4 {
        1.0 / (2.0 * m as f64 + 1.0)
    } else {
        boys_gamma_reference(m, x)
    }
}

#[cfg(test)]
fn boys_exact_reference(m: u64, x: f64) -> f64 {
    if x == 0.0 {
        return 1.0 / (2 * m + 1) as f64;
    }
    boys_gamma_reference(m, x)
}

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

    #[test]
    fn test_sqrt_i() {
        assert_abs_diff_eq!(SQRT_PI, PI.sqrt(), epsilon = 1e-14);
    }

    #[test]
    fn test_boys_function() {
        // Reference values from reliable sources or verified calculations.
        let test_cases = [
            (0, 0.001, 0.9996667666428618),
            (0, 0.5, 0.855624),
            (3, 0.01, 0.14175056440779324),
            (5, 0.015, 0.08976271177772857),
            (0, 1.0, 0.7468241328124271),
            (2, 5.0, 0.010995436178434296),
            (4, 10.0, 0.00018061943636439907),
            (3, 7.5, 0.0013864655818003292),
            (0, 25.0, 0.17724538509027907),
            (2, 50.0, 3.7599424119465e-05),
            (5, 100.0, 2.6171388894056747e-10),
        ];
        for (m, x, expected) in test_cases {
            let result = boys_function(m, x);
            assert_abs_diff_eq!(result, expected, epsilon = 1e-6);
        }
    }

    #[test]
    fn optimized_boys_function_matches_exact_reference() {
        let x_values = [
            0.0, 1e-12, 1e-9, 1e-6, 1e-4, 1e-3, 0.01, 0.1, 0.2, 0.5, 1.0, 1.25, 1.5, 2.0, 3.0, 5.0,
            10.0, 25.0, 50.0, 100.0,
        ];

        for m in 0..=12 {
            for x in x_values {
                let result = boys_function(m, x);
                let expected = if m > 0 && x.abs() < (m as f64 + 0.5) * 1e-4 {
                    boys_function_compatible(m, x)
                } else {
                    boys_exact_reference(m, x)
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
    fn cached_boys_function_matches_compatible_values() {
        let x_values = [
            0.0, 1e-12, 1e-9, 1e-6, 1e-4, 1e-3, 0.01, 0.1, 0.2, 0.5, 1.0, 1.25, 1.5, 2.0, 3.0, 5.0,
            10.0, 25.0, 50.0, 100.0,
        ];

        for max_order in 0..=12 {
            for x in x_values {
                let cache = CachedBoysFunction::new(max_order, x);
                for m in 0..=max_order {
                    let result = cache[m];
                    let expected = boys_function_compatible(m as u64, x);
                    let error = (result - expected).abs();
                    assert!(
                    error <= 1e-9,
                        "max_order={max_order}, m={m}, x={x}, result={result}, expected={expected}, error={error}"
                    );
                }
            }
        }
    }
}
