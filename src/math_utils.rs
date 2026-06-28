use factorial::Factorial;
#[cfg(test)]
use nalgebra::DMatrix;
pub mod boys;
pub use boys::boys_function;

pub(crate) mod F64Const {
    pub const SQRT_PI: f64 = 1.772_453_850_905_519;
    #[cfg(test)]
    mod tests {
        use approx::assert_abs_diff_eq;
        use super::SQRT_PI;
        use std::f64::consts::PI;

        #[test]
        fn test_sqrt_pi() {
            assert_abs_diff_eq!(SQRT_PI, PI.sqrt(), epsilon = 1e-14);
        }
    }
}

#[allow(dead_code)]
pub fn binomial(n: u64, k: u64) -> u64 {
    if k > n {
        0
    } else {
        n.factorial() / (k.factorial() * (n - k).factorial())
    }
}

#[allow(dead_code)]
pub fn hermite_classic(n: u64, x: f64) -> f64 {
    let mut sum = 0.0;

    for m in 0..=(n / 2) {
        // Corrected coefficient calculation with proper parentheses
        let coefficient = n.factorial() / (m.factorial() * (n - 2 * m).factorial());
        let term = coefficient as f64
            * (-1.0_f64).powi(m as i32)
            * 2.0_f64.powi((n as i32) - (2 * m) as i32)
            * x.powi((n as i32) - (2 * m) as i32);
        sum += term;
    }

    sum
}

#[allow(dead_code, non_snake_case)]
pub fn hermite(n: u64, PA: u32, QA: u32, p: f64, q: f64, f0: f64) -> f64 {
    let x = (PA as f64 - QA as f64) / (p + q).sqrt();
    f0 * hermite_classic(n, x)
}

#[macro_export]
#[cfg(debug_assertions)]
macro_rules! debug_assert_is_symmetric {
    ($matrix:expr, $tol:expr) => {{
        let matrix = $matrix;
        let tol = $tol;
        let n = matrix.nrows();
        debug_assert_eq!(
            n,
            matrix.ncols(),
            "Une matrice non carrée ne peut pas être symétrique"
        );

        for i in 0..n {
            for j in 0..i {
                let delta: f64 = matrix[(i, j)] - matrix[(j, i)];
                debug_assert!(
                    delta.abs() < tol,
                    "La matrice n'est pas symétrique : S[{},{}] = {}, mais S[{},{}] = {}",
                    i,
                    j,
                    matrix[(i, j)],
                    j,
                    i,
                    matrix[(j, i)]
                );
            }
        }
    }};
}

#[macro_export]
#[cfg(not(debug_assertions))]
macro_rules! debug_assert_is_symmetric {
    ($matrix:expr, $tol:expr) => {{}};
}

#[cfg(test)]
pub(crate) fn is_positive_definite(matrix: &DMatrix<f64>) -> bool {
    matrix
        .clone()
        .symmetric_eigen()
        .eigenvalues
        .iter()
        .all(|&v| v > 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;
    use nalgebra::DMatrix;

    #[test]
    fn test_binomial() {
        assert_eq!(binomial(5, 2), 10); // 5! / (2! * (5-2)!) = 10
        assert_eq!(binomial(6, 3), 20); // 6! / (3! * (6-3)!) = 20
        assert_eq!(binomial(10, 0), 1); // 10! / (0! * (10-0)!) = 1
        assert_eq!(binomial(0, 0), 1); // 0! / (0! * (0-0)!) = 1
        assert_eq!(binomial(5, 6), 0); // k > n, so result = 0
    }

    #[test]
    fn test_hermite_classic() {
        // Hermite polynomials (Physicist's version)

        // H_0(x) = 1
        assert_eq!(hermite_classic(0, 0.0), 1.0);

        // H_1(x) = 2x
        assert_eq!(hermite_classic(1, 1.0), 2.0);
        assert_eq!(hermite_classic(1, -1.0), -2.0);

        // H_2(x) = 4x^2 - 2
        assert_abs_diff_eq!(hermite_classic(2, 1.0), 2.0, epsilon = 1e-6); // Corrected
        assert_abs_diff_eq!(hermite_classic(2, 0.0), -2.0, epsilon = 1e-6); // Corrected

        // H_3(x) = 8x^3 - 12x
        assert_abs_diff_eq!(hermite_classic(3, 1.0), -4.0, epsilon = 1e-6);
        assert_abs_diff_eq!(hermite_classic(3, 0.0), 0.0, epsilon = 1e-6);

        // H_4(x) = 16x^4 - 48x^2 + 12
        assert_abs_diff_eq!(hermite_classic(4, 1.0), -20.0, epsilon = 1e-6);
        assert_abs_diff_eq!(hermite_classic(4, 0.0), 12.0, epsilon = 1e-6);
    }

    #[test]
    fn test_is_symmetric() {
        #[cfg(debug_assertions)]
        {
            let mat = DMatrix::from_row_slice(2, 2, &[1.0, 2.0, 2.0, 1.0]);
            crate::debug_assert_is_symmetric!(&mat, 1e-12); // Should not fail

            let mat_non_sym = DMatrix::from_row_slice(2, 2, &[1.0, 3.0, 2.0, 1.0]); // Corrected to make it non-symmetric
            let result =
                std::panic::catch_unwind(|| crate::debug_assert_is_symmetric!(&mat_non_sym, 1e-12));
            assert!(result.is_err(), "The matrix should not be symmetric");
        }
    }

    #[test]
    fn test_is_positive_definite() {
        let mat = DMatrix::from_row_slice(2, 2, &[2.0, 1.0, 1.0, 2.0]);
        assert!(is_positive_definite(&mat));

        let mat_not_pd = DMatrix::from_row_slice(2, 2, &[1.0, 1.0, 1.0, 1.0]);
        assert!(!is_positive_definite(&mat_not_pd));
    }
}
