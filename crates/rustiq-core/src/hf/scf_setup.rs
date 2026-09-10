use super::orthogonalization::OrthogonalizationInfo;

/// A scientific stage performed while preparing an SCF calculation.
///
/// This deliberately carries no presentation text: frontends choose how to
/// describe these stages to their users.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScfSetupStep {
    CoreHamiltonian,
    OverlapMatrix,
    OverlapOrthogonalizer,
    OverlapOrthogonalized(OrthogonalizationInfo),
    ElectronRepulsionIntegrals,
    InitialDensityGuess,
}
