use super::orthogonalization::OrthogonalizationInfo;
use super::scf_iteration::ScfIteration;

/// A scientific stage performed while preparing an SCF calculation.
///
/// This deliberately carries no presentation text: frontends choose how to
/// describe these stages to their users.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScfSetupStep {
    CoreHamiltonian,
    OverlapMatrix,
    OverlapOrthogonalizer,
    OverlapOrthogonalized(OrthogonalizationInfo),
    ElectronRepulsionIntegrals,
    InitialDensityGuess,
}

pub trait ScfObserver {
    fn on_iteration(&mut self, iteration: &ScfIteration);
}

#[cfg(test)]
#[derive(Default)]
pub(crate) struct RecordingScfObserver(pub Vec<ScfIteration>);

#[cfg(test)]
impl ScfObserver for RecordingScfObserver {
    fn on_iteration(&mut self, iteration: &ScfIteration) {
        self.0.push(iteration.clone());
    }
}

impl ScfObserver for Box<dyn ScfObserver> {
    fn on_iteration(&mut self, iteration: &ScfIteration) {
        self.as_mut().on_iteration(iteration);
    }
}

#[allow(dead_code)]
pub struct NoopScfObserver;

impl ScfObserver for NoopScfObserver {
    fn on_iteration(&mut self, _iteration: &ScfIteration) {}
}
