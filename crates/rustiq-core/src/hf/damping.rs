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
