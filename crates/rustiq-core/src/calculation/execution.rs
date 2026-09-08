use std::time::Duration;

use super::{CalculationError, ScfSetupStep};
use crate::{
    basis::gaussian::basis::Basis,
    config::{HfConfig, ResolvedHfMethod},
    hf::{scf_iteration::ScfIteration, scf_observer::ScfObserver, scf_result::ScfResult},
    mp2::Mp2Result,
};

#[derive(Debug, Clone)]
pub struct HfCalculationResult {
    pub method: ResolvedHfMethod,
    pub scf: ScfResult,
}

/// Results of the requested scientific stages, without presentation choices.
#[derive(Debug, Clone, Default)]
pub struct CalculationResult {
    pub hf: Option<HfCalculationResult>,
    pub mp2: Option<Mp2Result>,
}

/// Notifications for frontends. Observation does not control scientific stages.
///
/// Frontends own rendering and may retain/report their own output errors, as
/// with `ScfObserver`. Completed HF is reported before attempting optional MP2.
pub trait CalculationObserver: ScfObserver {
    fn on_basis_start(&mut self) {}
    fn on_basis_ready(&mut self, _basis: &Basis, _elapsed: Duration) {}
    fn on_hf_start(&mut self, _method: ResolvedHfMethod, _config: &HfConfig) {}
    fn on_scf_setup_step(&mut self, _step: ScfSetupStep) {}
    fn on_hf_complete(&mut self, _result: &HfCalculationResult) {}
    fn on_mp2_complete(&mut self, _hf: &HfCalculationResult, _result: &Mp2Result) {}
}

pub struct NoopCalculationObserver;

impl ScfObserver for NoopCalculationObserver {
    fn on_iteration(&mut self, _iteration: &ScfIteration) {}
}
impl CalculationObserver for NoopCalculationObserver {}

/// Shared execution interface for builders and prepared calculations.
pub trait CalculationExecution {
    fn execute(&self) -> Result<CalculationResult, CalculationError> {
        self.execute_with_observer(&mut NoopCalculationObserver)
    }

    fn execute_with_observer(
        &self,
        observer: &mut impl CalculationObserver,
    ) -> Result<CalculationResult, CalculationError>;
}
