use std::io::{self, Write};

use miette::IntoDiagnostic;
use serde::Serialize;

use rustiq_core::{
    calculation::{Mp2Result, OrthogonalizationInfo, ScfResult, SpinDiagnostics},
    config::ResolvedHfMethod,
};

use crate::cli::{
    commands::{
        batch_orchestration::{CampaignEvent, CampaignResult, ExecutionResult},
        calculation_result::CalculationSummary,
    },
    ux::bat,
};

pub(crate) fn print_calculation(result: &CalculationSummary, pretty: bool) -> miette::Result<()> {
    let output = CalculationOutput::from(result);
    output.ensure_finite().into_diagnostic()?;
    print_json(&output, pretty)
}

pub(crate) fn on_campaign_event<E: std::fmt::Display>(
    event: &CampaignEvent<'_, CalculationSummary, E>,
    pretty: bool,
) -> miette::Result<()> {
    let CampaignEvent::Completed(campaign) = event;
    let output = BatchOutput::from_campaign(campaign).into_diagnostic()?;
    print_json(&output, pretty)
}

impl From<&CalculationSummary> for CalculationOutput {
    fn from(result: &CalculationSummary) -> Self {
        let mut output = Self::new(
            result.hf.method,
            &result.hf.scf,
            result.converged,
            result.mp2.as_ref(),
        );
        output.artifacts.clone_from(&result.artifacts);
        output
    }
}

/// Serialize before writing to stdout so a serialization error cannot emit partial JSON.
fn serialize_json<T: Serialize>(value: &T, pretty: bool) -> serde_json::Result<Vec<u8>> {
    if pretty {
        serde_json::to_vec_pretty(value)
    } else {
        serde_json::to_vec(value)
    }
}

/// Use the same JSON presentation for single calculations and Nickel batches.
pub(crate) fn print_json<T: Serialize>(value: &T, pretty: bool) -> miette::Result<()> {
    let json = serialize_json(value, pretty).into_diagnostic()?;
    if pretty {
        bat::print_json(&json)
    } else {
        let mut stdout = io::stdout().lock();
        stdout.write_all(&json).into_diagnostic()?;
        writeln!(stdout).into_diagnostic()
    }
}

/// Version 1 of `RustiQ`'s stable, machine-readable calculation-output contract.
#[derive(Debug, Serialize)]
pub(crate) struct CalculationOutput {
    pub schema_version: u32,
    pub calculation: CalculationResultOutput,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<Vec<crate::cli::commands::artifact_session::ArtifactReport>>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CalculationResultOutput {
    pub hf: HfResultOutput,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mp2: Option<Mp2ResultOutput>,
}

#[derive(Debug, Serialize)]
pub(crate) struct HfResultOutput {
    pub method: &'static str,
    pub converged: bool,
    pub iterations: usize,
    pub electronic_energy: f64,
    pub nuclear_repulsion_energy: f64,
    pub total_energy: f64,
    pub delta_energy: f64,
    pub residual_norm: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spin: Option<SpinDiagnosticsOutput>,
    pub orthogonalization: OrthogonalizationOutput,
}

#[derive(Debug, Serialize)]
pub(crate) struct SpinDiagnosticsOutput {
    pub s_squared: f64,
    pub ideal_s_squared: f64,
    pub spin_contamination: f64,
}

#[derive(Debug, Serialize)]
pub(crate) struct OrthogonalizationOutput {
    pub ao_basis_dimension: usize,
    pub effective_rank: usize,
    pub discarded_directions: usize,
    pub relative_linear_dependency_threshold: f64,
}

#[derive(Debug, Serialize)]
pub(crate) struct Mp2ResultOutput {
    pub method: &'static str,
    pub correlation_energy: f64,
    pub electronic_energy: f64,
    pub total_energy: f64,
}

impl CalculationOutput {
    pub(crate) fn new(
        method: ResolvedHfMethod,
        hf: &ScfResult,
        converged: bool,
        mp2: Option<&Mp2Result>,
    ) -> Self {
        let mp2_method = match method {
            ResolvedHfMethod::Rhf => "RHF-MP2",
            ResolvedHfMethod::Uhf => "UHF-MP2",
        };
        Self {
            schema_version: 1,
            artifacts: None,
            calculation: CalculationResultOutput {
                hf: HfResultOutput::from((method, hf, converged)),
                mp2: mp2.map(|result| Mp2ResultOutput {
                    method: mp2_method,
                    correlation_energy: result.correlation_energy,
                    electronic_energy: result.electronic_energy,
                    total_energy: result.electronic_energy + hf.nuclear_repulsion_energy,
                }),
            },
        }
    }

    /// Reject values that JSON would silently convert to null before serializing.
    pub(crate) fn ensure_finite(&self) -> Result<(), serde_json::Error> {
        let hf = &self.calculation.hf;
        let mut values = vec![
            hf.electronic_energy,
            hf.nuclear_repulsion_energy,
            hf.total_energy,
            hf.delta_energy,
            hf.residual_norm,
            hf.orthogonalization.relative_linear_dependency_threshold,
        ];
        if let Some(spin) = &hf.spin {
            values.extend([
                spin.s_squared,
                spin.ideal_s_squared,
                spin.spin_contamination,
            ]);
        }
        if let Some(mp2) = &self.calculation.mp2 {
            values.extend([
                mp2.correlation_energy,
                mp2.electronic_energy,
                mp2.total_energy,
            ]);
        }
        if values.into_iter().all(f64::is_finite) {
            Ok(())
        } else {
            Err(<serde_json::Error as serde::ser::Error>::custom(
                "calculation result contains a non-finite floating-point value",
            ))
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct BatchOutput {
    pub schema_version: u32,
    pub kind: &'static str,
    pub calculations: Vec<BatchEntry>,
}

impl BatchOutput {
    fn from_campaign<E: std::fmt::Display>(
        campaign: &CampaignResult<CalculationSummary, E>,
    ) -> serde_json::Result<Self> {
        let calculations = campaign
            .results
            .iter()
            .map(|outcome| {
                let outcome = match outcome {
                    Ok(result) => {
                        let output = CalculationOutput::from(result);
                        output.ensure_finite()?;
                        if result.is_converged() {
                            BatchOutcome::Success { result: output }
                        } else {
                            BatchOutcome::NonConverged { result: output }
                        }
                    }
                    Err(error) => BatchOutcome::Error {
                        error: BatchError {
                            message: error.to_string(),
                        },
                    },
                };
                Ok(BatchEntry { outcome })
            })
            .collect::<serde_json::Result<Vec<_>>>()?;
        Ok(Self {
            schema_version: 1,
            kind: "batch",
            calculations,
        })
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct BatchEntry {
    #[serde(flatten)]
    pub outcome: BatchOutcome,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum BatchOutcome {
    Success { result: CalculationOutput },
    NonConverged { result: CalculationOutput },
    Error { error: BatchError },
}

#[derive(Debug, Serialize)]
pub(crate) struct BatchError {
    pub message: String,
}

impl From<(ResolvedHfMethod, &ScfResult, bool)> for HfResultOutput {
    fn from((method, result, converged): (ResolvedHfMethod, &ScfResult, bool)) -> Self {
        Self {
            method: match method {
                ResolvedHfMethod::Rhf => "RHF",
                ResolvedHfMethod::Uhf => "UHF",
            },
            converged,
            iterations: result.iterations,
            electronic_energy: result.electronic_energy,
            nuclear_repulsion_energy: result.nuclear_repulsion_energy,
            total_energy: result.total_energy,
            delta_energy: result.delta_energy,
            residual_norm: result.residual_norm,
            spin: result.spin.map(SpinDiagnosticsOutput::from),
            orthogonalization: OrthogonalizationOutput::from(result.orthogonalization),
        }
    }
}

impl From<SpinDiagnostics> for SpinDiagnosticsOutput {
    fn from(spin: SpinDiagnostics) -> Self {
        Self {
            s_squared: spin.s_squared,
            ideal_s_squared: spin.ideal_s_squared,
            spin_contamination: spin.spin_contamination,
        }
    }
}

impl From<OrthogonalizationInfo> for OrthogonalizationOutput {
    fn from(info: OrthogonalizationInfo) -> Self {
        Self {
            ao_basis_dimension: info.basis_dimension,
            effective_rank: info.effective_rank,
            discarded_directions: info.discarded_directions,
            relative_linear_dependency_threshold: info.relative_threshold,
        }
    }
}

#[cfg(test)]
mod tests {
    use approx::assert_abs_diff_eq;

    use super::*;
    use rustiq_core::calculation::{OrthogonalizationInfo, ScfEnergyDetails, ScfTimings};

    fn scf_result() -> ScfResult {
        ScfResult {
            iterations: 2,
            electronic_energy: -1.831_863_646_477_507,
            nuclear_repulsion_energy: 0.715_104_339_081_081,
            total_energy: -1.116_759_307_396_426,
            delta_energy: 0.0,
            residual_norm: 0.0,
            spin: None,
            energy_details: ScfEnergyDetails {
                kinetic_energy: 0.0,
                nuclear_attraction_energy: 0.0,
                electron_repulsion_energy: 0.0,
            },
            orthogonalization: OrthogonalizationInfo {
                basis_dimension: 2,
                effective_rank: 1,
                discarded_directions: 1,
                relative_threshold: 1e-8,
            },
            timings: ScfTimings::default(),
        }
    }

    #[test]
    fn event_collector_preserves_final_result_document() {
        use rustiq_core::{
            basis::BasisFile,
            calculation::{CalculationBuilder, CalculationExecution},
            config::{HfConfig, Mp2Config},
            molecules::geometry::Geometry,
        };
        let geometry =
            Geometry::from_reader(&include_bytes!("../../../samples/h2/molecule.xyz")[..]).unwrap();
        let basis = BasisFile::from_reader(
            &include_bytes!("../../../crates/rustiq-core/tests/data/sto-3g.json")[..],
        )
        .unwrap();
        for (max_iterations, mp2) in [(100, false), (100, true), (1, false)] {
            let mut collector =
                crate::cli::commands::calculation_result::CalculationSummaryCollector::default();
            let calculation = CalculationBuilder::new(&geometry, &basis)
                .with_hf(HfConfig {
                    max_iterations: std::num::NonZeroUsize::new(max_iterations).unwrap(),
                    ..HfConfig::default()
                })
                .with_mp2(mp2.then(Mp2Config::default));
            let result = calculation
                .execute_with_events(|event| collector.on_event(&event))
                .unwrap();
            assert_eq!(result.hf.is_converged(), max_iterations > 1);
            let hf = result.hf.summary();
            let expected = CalculationOutput::new(
                hf.method,
                &hf.scf,
                result.hf.is_converged(),
                result.mp2.as_ref(),
            );
            assert_eq!(
                serde_json::to_value(CalculationOutput::from(&collector.finish())).unwrap(),
                serde_json::to_value(expected).unwrap(),
            );
        }
    }

    #[test]
    fn json_output_is_valid_and_preserves_hf_values() {
        let output = CalculationOutput::new(ResolvedHfMethod::Rhf, &scf_result(), true, None);
        let bytes = serialize_json(&output, false).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["calculation"]["hf"]["method"], "RHF");
        assert_abs_diff_eq!(
            value["calculation"]["hf"]["total_energy"].as_f64().unwrap(),
            -1.116_759_307_396_426,
            epsilon = 1e-15
        );
        assert_eq!(
            value["calculation"]["hf"]["orthogonalization"]["effective_rank"],
            1
        );
        assert!(value["calculation"].get("mp2").is_none());
    }

    #[test]
    fn json_output_identifies_uhf_and_mp2() {
        let mp2 = Mp2Result {
            correlation_energy: -0.013_138_073_589_533,
            electronic_energy: -1.845_001_720_067_04,
        };
        let output = CalculationOutput::new(ResolvedHfMethod::Uhf, &scf_result(), true, Some(&mp2));
        let value = serde_json::to_value(&output).unwrap();

        assert_eq!(value["calculation"]["hf"]["method"], "UHF");
        assert_eq!(value["calculation"]["mp2"]["method"], "UHF-MP2");
        assert_abs_diff_eq!(
            value["calculation"]["mp2"]["correlation_energy"]
                .as_f64()
                .unwrap(),
            mp2.correlation_energy,
            epsilon = 1e-15
        );
    }

    #[test]
    fn json_output_rejects_non_finite_values() {
        let mut result = scf_result();
        result.total_energy = f64::NAN;
        let output = CalculationOutput::new(ResolvedHfMethod::Rhf, &result, true, None);
        assert!(output.ensure_finite().is_err());
    }

    #[test]
    fn json_output_rejects_non_finite_spin() {
        let mut result = scf_result();
        result.spin = Some(SpinDiagnostics {
            s_squared: f64::NAN,
            ideal_s_squared: 0.75,
            spin_contamination: 0.0,
        });
        let output = CalculationOutput::new(ResolvedHfMethod::Uhf, &result, true, None);
        assert!(output.ensure_finite().is_err());
    }

    fn summary(converged: bool) -> CalculationSummary {
        CalculationSummary {
            hf: rustiq_core::calculation::HfCalculationResult {
                method: ResolvedHfMethod::Rhf,
                scf: scf_result(),
            },
            converged,
            mp2: None,
            artifacts: None,
        }
    }

    #[test]
    fn campaign_formatter_builds_ordered_outcomes_and_preserves_artifacts() {
        let mut success = summary(true);
        success.artifacts = Some(vec![
            crate::cli::commands::artifact_session::ArtifactReport {
                name: "ao_eri",
                decision: "reused",
                origin: "bundle",
                source_index: Some(2),
            },
        ]);
        let campaign = CampaignResult {
            results: vec![Ok(success), Ok(summary(false)), Err("calculation failed")],
        };
        let output = BatchOutput::from_campaign(&campaign).unwrap();
        for pretty in [false, true] {
            let bytes = serialize_json(&output, pretty).unwrap();
            let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(value["schema_version"], 1);
            assert_eq!(value["kind"], "batch");
            assert_eq!(value["calculations"][0]["status"], "success");
            assert_eq!(value["calculations"][1]["status"], "non_converged");
            assert_eq!(value["calculations"][2]["status"], "error");
            assert_eq!(
                value["calculations"][2]["error"]["message"],
                "calculation failed"
            );
            assert_eq!(
                value["calculations"][0]["result"]["artifacts"][0]["source_index"],
                2
            );
        }
        assert_eq!(
            campaign.summary(),
            crate::cli::commands::batch_orchestration::BatchSummary {
                succeeded: 1,
                non_converged: 1,
                failed: 1,
            }
        );
    }

    #[test]
    fn batch_cannot_turn_non_finite_result_into_json_null() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut result = summary(false);
            result.hf.scf.residual_norm = value;
            assert!(result.ensure_finite().is_err());
            let campaign = CampaignResult::<_, &str> {
                results: vec![Ok(summary(true)), Ok(result)],
            };
            assert!(BatchOutput::from_campaign(&campaign).is_err());
        }
    }
}
