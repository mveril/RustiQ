use factorial::{DoubleFactorial, Factorial};
use special::{Gamma, Primitive};
use std::f64::consts::PI;

/// Calculate the Boys function F_m(x) for a given order m and parameter x.
pub fn boys_function(m: u64, x: f64) -> f64 {
    if x == 0.0 {
        1.0 / (2 * m + 1) as f64
    } else if m == 0 {
        let sqrtx = x.sqrt();
        (PI.sqrt() * sqrtx.erf()) / (2.0 * sqrtx)
    } else {
        if x.abs() < (m as f64 + 0.5) * 1e-4 {
            boys_small_x(m)
        // } else if x > 20.0 {
        //     boys_large_x(m, x)
        } else {
            boys_intermediate_x(m, x)
        }
    }
}

/// Approximation for small x.
fn boys_small_x(m: u64) -> f64 {
    1.0 / (2.0 * m as f64 + 1.0)
}

/// Approximation for large x based on asymptotic expansion.
fn boys_large_x(m: u64, x: f64) -> f64 {
    let double_factorial = (2 * m - 1).double_factorial() as f64;
    double_factorial / (2.0_f64.powi(m as i32 + 1) * x.powf(m as f64 + 0.5))
}

/// Calculation using the incomplete gamma function from `rgsl`.
fn boys_intermediate_x(m: u64, x: f64) -> f64 {
    let a = m as f64 + 0.5;
    let gamma_a = a.gamma(); // Compute Γ(a)
    let p_lower_gamma = x.inc_gamma(a); // Compute P(x, a) = γ(x, a) / Γ(a)
    let gamma_inc = p_lower_gamma * gamma_a; // Compute γ(x, a)
    gamma_inc / (2.0 * x.powf(a))
}

#[cfg(test)]
mod tests {
    use super::boys_function;
    use approx::assert_abs_diff_eq;

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
}
