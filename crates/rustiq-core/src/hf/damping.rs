//! Energy minimization along a common density segment for RHF and UHF.

use nalgebra::DMatrix;

use super::numerical_error::{ensure_finite_value, NumericalError};

pub(super) struct DensitySegment<'a> {
    pub(super) previous_density: &'a DMatrix<f64>,
    pub(super) density: &'a DMatrix<f64>,
    pub(super) previous_fock: &'a DMatrix<f64>,
    pub(super) fock: &'a DMatrix<f64>,
}

/// All channels use the same fraction, retaining the interspin Coulomb terms.
/// Each density uses its physical occupation convention (spin-summed for RHF).
/// The Fock matrices must be physical operators for their endpoint densities.
/// Population conservation additionally requires equally populated endpoints;
/// the SCF loops exclude the Zero startup sentinel before calling this routine.
pub(super) fn damping_fraction<'a>(
    channels: impl IntoIterator<Item = DensitySegment<'a>>,
) -> Result<f64, NumericalError> {
    // HF Fock operators are affine in the full density. Along a common
    // segment, \(E(t)-E(0)=\mathrm{slope}\,t+\mathrm{curvature}\,t^2\), exactly.
    let mut slope = 0.0;
    let mut curvature = 0.0;
    for channel in channels {
        let step = channel.density - channel.previous_density;
        slope += step.dot(channel.previous_fock);
        curvature += 0.5 * step.dot(&(channel.fock - channel.previous_fock));
    }
    ensure_finite_value(slope, "optimal damping slope")?;
    ensure_finite_value(curvature, "optimal damping curvature")?;
    Ok(optimal_damping_fraction(slope, curvature))
}

pub(super) fn optimal_damping_fraction(slope: f64, curvature: f64) -> f64 {
    if curvature > 0.0 {
        (-0.5 * (slope / curvature)).clamp(0.0, 1.0)
    } else if slope + curvature <= 0.0 {
        1.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn tiny_curvature_needs_no_arbitrary_cutoff() {
        for scale in [1.0, f64::EPSILON, f64::MIN_POSITIVE, f64::from_bits(8)] {
            assert_abs_diff_eq!(optimal_damping_fraction(-scale, scale), 0.5, epsilon = 0.0);
            assert_abs_diff_eq!(
                optimal_damping_fraction(-3.0 * scale, scale),
                1.0,
                epsilon = 0.0
            );
            assert_abs_diff_eq!(optimal_damping_fraction(scale, scale), 0.0, epsilon = 0.0);
        }
        // A finite slope divided by subnormal curvature may overflow; the
        // correctly clamped minimizer is still the appropriate endpoint.
        assert_abs_diff_eq!(
            optimal_damping_fraction(-1.0, f64::from_bits(1)),
            1.0,
            epsilon = 0.0
        );
        assert_abs_diff_eq!(
            optimal_damping_fraction(1.0, f64::from_bits(1)),
            0.0,
            epsilon = 0.0
        );
        assert_abs_diff_eq!(optimal_damping_fraction(0.0, 0.0), 1.0, epsilon = 0.0);
        assert_abs_diff_eq!(optimal_damping_fraction(2.0, -2.0), 1.0, epsilon = 0.0);
    }

    #[test]
    fn rejects_nonfinite_slopes_and_curvatures() {
        let zero = DMatrix::zeros(1, 1);
        let one = DMatrix::identity(1, 1);
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let nonfinite = DMatrix::from_element(1, 1, value);
            for (previous_fock, fock) in [(&nonfinite, &one), (&zero, &nonfinite)] {
                assert!(damping_fraction([DensitySegment {
                    previous_density: &zero,
                    density: &one,
                    previous_fock,
                    fock,
                }])
                .is_err());
            }
        }
        let huge = DMatrix::from_element(1, 1, f64::MAX);
        assert!(damping_fraction([DensitySegment {
            previous_density: &zero,
            density: &one,
            previous_fock: &huge,
            fock: &(-&huge),
        }])
        .is_err());
    }
}
