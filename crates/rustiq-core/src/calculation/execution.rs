use std::time::Duration;

use super::{CalculationError, ScfSetupStep};
use crate::{
    basis::Basis,
    calculation::{ScfIteration, ScfResult},
    config::{HfConfig, ResolvedHfMethod},
    mp2::Mp2Result,
};

#[derive(Debug, Clone)]
pub struct HfCalculationResult {
    pub method: ResolvedHfMethod,
    pub scf: ScfResult,
}

/// Results of the requested scientific stages, without presentation choices.
#[derive(Debug, Clone)]
pub struct CalculationResult {
    pub hf: HfCalculationResult,
    pub mp2: Option<Mp2Result>,
}

/// Synchronous notifications in scientific execution order.
///
/// References are valid during the callback; retaining data requires copying it.
/// Basis preparation precedes HF setup and iterations (including finalization).
/// Completed HF is reported before optional MP2, even if MP2 subsequently fails.
/// Scientific errors are returned by execution, not emitted as events. Frontends
/// retain their own rendering errors; callbacks do not control execution.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum CalculationEvent<'a> {
    BasisStarted,
    BasisReady {
        basis: &'a Basis,
        elapsed: Duration,
    },
    HfStarted {
        method: ResolvedHfMethod,
        config: &'a HfConfig,
    },
    ScfSetup(ScfSetupStep),
    ScfIteration(&'a ScfIteration),
    HfCompleted(&'a HfCalculationResult),
    Mp2Completed {
        hf: &'a HfCalculationResult,
        result: &'a Mp2Result,
    },
}

/// Shared execution interface for builders and prepared calculations.
pub trait CalculationExecution {
    fn execute(&self) -> Result<CalculationResult, CalculationError> {
        self.execute_with_events(|_| {})
    }

    fn execute_with_events(
        &self,
        events: impl FnMut(CalculationEvent<'_>),
    ) -> Result<CalculationResult, CalculationError>;
}
