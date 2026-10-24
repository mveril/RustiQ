//! Energy minimization along a common density segment for RHF and UHF.

use nalgebra::DMatrix;

use super::numerical_error::{ensure_finite_value, NumericalError};

type DensitySegment<'a> = (
    &'a DMatrix<f64>,
    &'a DMatrix<f64>,
    &'a DMatrix<f64>,
    &'a DMatrix<f64>,
);

/// All channels use the same fraction, retaining the interspin Coulomb terms.
/// Each density uses its physical occupation convention (spin-summed for RHF).
pub(super) fn damping_fraction<const N: usize>(
    channels: [DensitySegment<'_>; N],
) -> Result<f64, NumericalError> {
    // HF Fock operators are affine in the full density. Along a common
    // segment, E(t) - E(0) = slope * t + curvature * t^2 exactly.
    let mut slope = 0.0;
    let mut curvature = 0.0;
    for (previous_density, density, previous_fock, fock) in channels {
        let step = density - previous_density;
        slope += step.dot(previous_fock);
        curvature += 0.5 * step.dot(&(fock - previous_fock));
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
