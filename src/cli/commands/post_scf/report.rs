use std::{
    io::{self, Write},
    path::Path,
};

use miette::{Diagnostic, IntoDiagnostic, NamedSource, Report};
use serde::{Deserialize, Serialize};

use super::{Campaign, Outcome, ScientificResult, Skip, Summary};
use crate::cli::{
    color,
    commands::run_command::CalculationOutputFormat,
    ux::{
        diagnostics::{emit_warning, with_code},
        json_output,
    },
};

const SKIPPED_CODE: &str = "rustiq::post_scf::skipped";
const FAILED_CODE: &str = "rustiq::post_scf::failed";

#[derive(Debug, thiserror::Error, Diagnostic)]
#[error("Calculation {source_index}, method {method}: {}", skip.reason)]
#[diagnostic(code(rustiq::post_scf::skipped), severity(Warning), help("{}", skip.help))]
struct Skipped {
    source_index: usize,
    method: &'static str,
    skip: Skip,
}

#[derive(Debug, Diagnostic)]
#[diagnostic(forward(error))]
struct Failed {
    source_index: usize,
    method: &'static str,
    error: Report,
}

impl std::fmt::Display for Failed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Calculation {}, method {}: {}",
            self.source_index, self.method, self.error
        )
    }
}
impl std::error::Error for Failed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.error.as_ref())
    }
}

#[derive(Serialize)]
struct JsonReport<'a> {
    schema_version: u32,
    summary: &'a Summary,
    calculations: Vec<JsonCalculation<'a>>,
}

#[derive(Serialize)]
struct JsonCalculation<'a> {
    source_index: usize,
    methods: Vec<JsonMethod<'a>>,
}

#[derive(Serialize)]
struct JsonMethod<'a> {
    method: &'static str,
    #[serde(flatten)]
    outcome: JsonOutcome<'a>,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum JsonOutcome<'a> {
    Succeeded { result: JsonScientificResult },
    Skipped { diagnostic: JsonDiagnostic<'a> },
    Failed { diagnostic: JsonDiagnostic<'a> },
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum JsonScientificResult {
    Mp2(JsonMp2Result),
    #[cfg(test)]
    RestrictedProbe {},
}

#[derive(Deserialize, Serialize)]
struct JsonMp2Result {
    reference: String,
    correlation_energy: f64,
    electronic_energy: f64,
    nuclear_repulsion_energy: f64,
    total_energy: f64,
}

#[derive(Serialize)]
struct JsonDiagnostic<'a> {
    code: String,
    message: std::borrow::Cow<'a, str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    help: Option<std::borrow::Cow<'a, str>>,
}

impl Campaign {
    pub(super) fn print(
        &self,
        format: CalculationOutputFormat,
        pretty: bool,
    ) -> miette::Result<()> {
        match format {
            CalculationOutputFormat::Text => {
                self.write_text(&mut color::stdout()).into_diagnostic()
            }
            CalculationOutputFormat::Json => json_output::print_plain_json(&self.json(), pretty),
        }
    }

    pub(super) fn emit_diagnostics(self, config: (&Path, &str)) -> miette::Result<()> {
        for calculation in self.calculations {
            let source_index = calculation.source_index;
            for (method, outcome) in calculation.methods {
                let method = method.name();
                match outcome {
                    Outcome::Skipped(skip) => emit_warning(Skipped {
                        source_index,
                        method,
                        skip,
                    })?,
                    Outcome::Failed(error) => {
                        let error = with_code(error, FAILED_CODE).with_source_code(
                            NamedSource::new(config.0.to_string_lossy(), config.1.to_owned()),
                        );
                        let report = Report::new(Failed {
                            source_index,
                            method,
                            error,
                        });
                        writeln!(io::stderr().lock(), "{report:?}").into_diagnostic()?;
                    }
                    Outcome::Succeeded(_) => {}
                }
            }
        }
        Ok(())
    }

    fn write_text(&self, writer: &mut impl Write) -> io::Result<()> {
        for calculation in &self.calculations {
            for (method, outcome) in &calculation.methods {
                if let Outcome::Succeeded(result) = outcome {
                    writeln!(
                        writer,
                        "Calculation {}, method {}: succeeded",
                        calculation.source_index,
                        method.name()
                    )?;
                    result.write_text(writer)?;
                }
            }
        }
        let Summary {
            succeeded,
            skipped,
            failed,
        } = self.summary;
        writeln!(
            writer,
            "Post-HF campaign: {succeeded} succeeded, {skipped} skipped, {failed} failed."
        )
    }

    fn json(&self) -> JsonReport<'_> {
        JsonReport {
            schema_version: 1,
            summary: &self.summary,
            calculations: self
                .calculations
                .iter()
                .map(|calculation| JsonCalculation {
                    source_index: calculation.source_index,
                    methods: calculation
                        .methods
                        .iter()
                        .map(|&(method, ref outcome)| JsonMethod {
                            method: method.name(),
                            outcome: JsonOutcome::from(outcome),
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

impl<'a> From<&'a Outcome> for JsonOutcome<'a> {
    fn from(outcome: &'a Outcome) -> Self {
        match outcome {
            Outcome::Succeeded(result) => Self::Succeeded {
                result: result.json(),
            },
            Outcome::Skipped(skip) => Self::Skipped {
                diagnostic: JsonDiagnostic {
                    code: SKIPPED_CODE.to_owned(),
                    message: skip.reason.as_str().into(),
                    help: Some(skip.help.into()),
                },
            },
            Outcome::Failed(error) => Self::Failed {
                diagnostic: JsonDiagnostic {
                    code: error
                        .code()
                        .map_or_else(|| FAILED_CODE.to_owned(), |code| code.to_string()),
                    message: error.to_string().into(),
                    help: error.help().map(|help| help.to_string().into()),
                },
            },
        }
    }
}

impl ScientificResult {
    fn json(&self) -> JsonScientificResult {
        match self {
            Self::Mp2(rustiq_core::persistence::PostScfMp2Result {
                reference,
                result,
                nuclear_repulsion_energy,
                ..
            }) => JsonScientificResult::Mp2(JsonMp2Result {
                reference: format!("{reference:?}").to_ascii_uppercase(),
                correlation_energy: result.correlation_energy,
                electronic_energy: result.electronic_energy,
                nuclear_repulsion_energy: *nuclear_repulsion_energy,
                total_energy: result.electronic_energy + nuclear_repulsion_energy,
            }),
            #[cfg(test)]
            Self::RestrictedProbe => JsonScientificResult::RestrictedProbe {},
        }
    }

    fn write_text(&self, writer: &mut impl Write) -> io::Result<()> {
        match self {
            Self::Mp2(rustiq_core::persistence::PostScfMp2Result {
                reference,
                result,
                nuclear_repulsion_energy,
                ..
            }) => {
                writeln!(writer, "  HF reference: {reference:?}")?;
                writeln!(
                    writer,
                    "  MP2 correlation energy: {:.6} Hartree",
                    result.correlation_energy
                )?;
                writeln!(
                    writer,
                    "  MP2 electronic energy: {:.6} Hartree",
                    result.electronic_energy
                )?;
                writeln!(
                    writer,
                    "  MP2 total energy: {:.6} Hartree",
                    result.electronic_energy + nuclear_repulsion_energy
                )
            }
            #[cfg(test)]
            Self::RestrictedProbe => Ok(()),
        }
    }
}

#[cfg(test)]
pub(super) fn skip_message(source_index: usize, method: super::Method, skip: Skip) -> String {
    Skipped {
        source_index,
        method: method.name(),
        skip,
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::JsonScientificResult;

    #[test]
    fn failure_codes_and_help_survive_json() {
        for (error, expected) in [
            (
                miette::miette!(code = "rustiq::specific", help = "retry", "failure"),
                "rustiq::specific",
            ),
            (miette::miette!("failure"), super::FAILED_CODE),
        ] {
            let terminal = super::Failed {
                source_index: 0,
                method: "mp2",
                error: crate::cli::ux::diagnostics::with_code(error, super::FAILED_CODE),
            };
            assert_eq!(
                miette::Diagnostic::code(&terminal).unwrap().to_string(),
                expected
            );
            assert_eq!(
                std::error::Error::source(&terminal).unwrap().to_string(),
                "failure"
            );
            let outcome = super::Outcome::Failed(terminal.error);
            let json = serde_json::to_value(super::JsonOutcome::from(&outcome)).unwrap();
            assert_eq!(json["diagnostic"]["code"], expected);
            assert_eq!(json["diagnostic"]["message"], "failure");
            if expected == "rustiq::specific" {
                assert_eq!(json["diagnostic"]["help"], "retry");
            }
        }
    }

    #[test]
    fn mp2_result_round_trips_without_an_extra_tag() {
        let expected = serde_json::json!({
            "reference": "RHF",
            "correlation_energy": -0.02,
            "electronic_energy": -1.8,
            "nuclear_repulsion_energy": 0.7,
            "total_energy": -1.1,
        });
        let result: JsonScientificResult = serde_json::from_value(expected.clone()).unwrap();
        assert!(matches!(result, JsonScientificResult::Mp2(_)));
        assert_eq!(serde_json::to_value(result).unwrap(), expected);
    }
}
