use std::io::{self, Write};

use miette::IntoDiagnostic;
use serde::Serialize;

use rustiq_core::{
    calculation::{
        CalculationResult, Mp2Result, OrthogonalizationInfo, ScfResult, SpinDiagnostics,
    },
    config::ResolvedHfMethod,
};

use crate::cli::{commands::batch_orchestration::ExecutionResult, ux::bat};

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

impl ExecutionResult for CalculationOutput {
    fn is_converged(&self) -> bool {
        self.calculation.hf.converged
    }
}

impl From<&CalculationResult> for CalculationOutput {
    fn from(result: &CalculationResult) -> Self {
        let hf = result.hf.summary();
        Self::new(
            hf.method,
            &hf.scf,
            result.hf.is_converged(),
            result.mp2.as_ref(),
        )
    }
}

/// Version 1 of `RustiQ`'s stable, machine-readable calculation-output contract.
#[derive(Debug, Serialize)]
pub(crate) struct CalculationOutput {
    pub schema_version: u32,
    pub calculation: CalculationResultOutput,
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

    #[test]
    fn batch_cannot_turn_non_finite_result_into_json_null() {
        let mut result = scf_result();
        result.residual_norm = f64::INFINITY;
        let output = CalculationOutput::new(ResolvedHfMethod::Rhf, &result, false, None);
        assert!(output.ensure_finite().is_err());
    }
}
