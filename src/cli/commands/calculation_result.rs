//! Presentation-independent summaries retained by the CLI, without scientific matrices.
use rustiq_core::calculation::{CalculationEvent, HfCalculationResult, Mp2Result};

use super::{artifact_session::ArtifactReport, batch_orchestration::ExecutionResult};

#[derive(Debug)]
pub(crate) struct CalculationSummary {
    pub hf: HfCalculationResult,
    pub converged: bool,
    pub mp2: Option<Mp2Result>,
    pub artifacts: Option<Vec<ArtifactReport>>,
}

impl ExecutionResult for CalculationSummary {
    fn is_converged(&self) -> bool {
        self.converged
    }
}

#[derive(Debug, thiserror::Error)]
#[error("calculation result contains a non-finite floating-point value")]
pub(crate) struct NonFiniteResult;

impl CalculationSummary {
    /// Validate numerical summaries independently of the selected output format.
    pub(crate) fn ensure_finite(&self) -> Result<(), NonFiniteResult> {
        let hf = &self.hf.scf;
        let mut values = [
            hf.electronic_energy,
            hf.nuclear_repulsion_energy,
            hf.total_energy,
            hf.delta_energy,
            hf.residual_norm,
            hf.orthogonalization.relative_threshold,
        ]
        .into_iter()
        .chain(hf.spin.into_iter().flat_map(|spin| {
            [
                spin.s_squared,
                spin.ideal_s_squared,
                spin.spin_contamination,
            ]
        }))
        .chain(self.mp2.iter().flat_map(|mp2| {
            [
                mp2.correlation_energy,
                mp2.electronic_energy,
                mp2.electronic_energy + hf.nuclear_repulsion_energy,
            ]
        }));
        if values.all(f64::is_finite) {
            Ok(())
        } else {
            Err(NonFiniteResult)
        }
    }
}

/// Only a completed calculation can populate the final summary.
#[derive(Default)]
pub(crate) struct CalculationSummaryCollector {
    result: Option<CalculationSummary>,
}

impl CalculationSummaryCollector {
    pub(crate) fn on_event(&mut self, event: &CalculationEvent<'_>) {
        if let CalculationEvent::CalculationCompleted(result) = event {
            self.result = Some(CalculationSummary {
                hf: result.hf.summary().clone(),
                converged: result.hf.is_converged(),
                mp2: result.mp2,
                artifacts: None,
            });
        }
    }

    /// Call only after successful core execution, which guarantees completion.
    pub(crate) fn finish(self) -> CalculationSummary {
        self.result
            .expect("successful calculation emits CalculationCompleted")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustiq_core::{
        basis::BasisFile,
        calculation::{CalculationBuilder, CalculationExecution},
        config::{HfConfig, Mp2Config},
        molecules::Geometry,
    };

    #[test]
    fn only_completed_requested_stages_populate_the_summary() {
        let geometry =
            Geometry::from_reader(&include_bytes!("../../../samples/h2/molecule.xyz")[..]).unwrap();
        let basis = BasisFile::from_reader(
            &include_bytes!("../../../crates/rustiq-core/tests/data/sto-3g.json")[..],
        )
        .unwrap();
        for (iterations, mp2) in [(100, false), (100, true), (1, false), (1, true)] {
            let mut collector = CalculationSummaryCollector::default();
            let mut completions = 0;
            let result = CalculationBuilder::new(&geometry, &basis)
                .with_hf(HfConfig {
                    max_iterations: std::num::NonZeroUsize::new(iterations).unwrap(),
                    ..HfConfig::default()
                })
                .with_mp2(mp2.then(Mp2Config::default))
                .execute_with_events(|event| {
                    if let CalculationEvent::CalculationCompleted(result) = &event {
                        assert_eq!(result.mp2.is_some(), mp2);
                        assert_eq!(result.hf.is_converged(), iterations > 1);
                        completions += 1;
                    }
                    collector.on_event(&event);
                });
            if iterations == 1 && mp2 {
                assert!(result.is_err());
                assert_eq!(completions, 0);
                assert!(collector.result.is_none());
            } else {
                assert!(result.is_ok());
                assert_eq!(completions, 1);
                let summary = collector.finish();
                assert_eq!(summary.converged, iterations > 1);
                assert_eq!(summary.mp2.is_some(), mp2);
            }
        }
    }
}
