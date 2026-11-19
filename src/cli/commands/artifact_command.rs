mod molden;

use std::{
    io::{self, Write},
    path::PathBuf,
};

use clap::Subcommand;
use miette::{miette, Diagnostic, IntoDiagnostic};
use rustiq_core::persistence::{ArtifactCheckStatus, RustiQBundle};
use serde::Serialize;
use thiserror::Error;

use super::{run_command::CalculationOutputFormat, CommandResult, Runnable};
use crate::cli::ux::{
    bat,
    calculation_presentation::{portable_requested_calculation, resolved_calculation},
};

#[derive(Debug, Subcommand)]
pub(crate) enum ArtifactCommands {
    /// Inspect versioned requested/resolved snapshots without executing calculations.
    Inspect(InspectCommand),
    /// Verify the stored payloads in a portable bundle.
    Verify(VerifyCommand),
    /// Export saved converged HF orbitals from a portable bundle.
    Export(molden::ExportCommand),
}

#[derive(Debug, clap::Args)]
pub(crate) struct InspectCommand {
    path: PathBuf,
    #[arg(long, value_enum, default_value_t = CalculationOutputFormat::Text)]
    format: CalculationOutputFormat,
}

#[derive(Debug, clap::Args)]
pub(crate) struct VerifyCommand {
    path: PathBuf,
    /// Return exit code 5 if artifacts are skipped and no verification fails.
    #[arg(long)]
    strict: bool,
    /// Select human-readable text or versioned JSON for the verification summary.
    #[arg(long, value_enum, default_value_t = CalculationOutputFormat::Text)]
    format: CalculationOutputFormat,
}

#[derive(Serialize)]
struct Inspection {
    schema_version: u32,
    kind: &'static str,
    payloads_verified: bool,
    sources: Vec<Source>,
    calculations: Vec<Entry>,
}

#[derive(Serialize)]
struct Source {
    original_name: String,
    size: usize,
}

#[derive(Serialize)]
struct Entry {
    source_index: usize,
    producer: Producer,
    requested: Canonical,
    resolved: Canonical,
    artifacts: Vec<Representation>,
}

#[derive(Serialize)]
struct Producer {
    name: String,
    version: String,
}
#[derive(Serialize)]
struct Canonical {
    configuration: String,
    geometry: String,
}
#[derive(Serialize)]
struct Representation {
    name: String,
    representation: String,
}

impl Runnable for ArtifactCommands {
    fn run(&self) -> CommandResult {
        match self {
            Self::Inspect(command) => command.run(),
            Self::Verify(command) => command.run(),
            Self::Export(command) => command.run(),
        }
    }
}

#[derive(Debug, Error, Diagnostic)]
#[error(
    "verification failed for calculation {calculation_index}, artifact {name} ({representation})"
)]
#[diagnostic(code(rustiq::artifact::verification_failed))]
struct VerificationFailure {
    calculation_index: usize,
    name: String,
    representation: String,
    #[source]
    #[diagnostic_source]
    source: Box<dyn Diagnostic + Send + Sync>,
}

#[derive(Debug, Error, Diagnostic)]
#[error("artifact verification failed")]
#[diagnostic(code(rustiq::artifact::verification_batch_failed))]
struct VerificationFailures {
    #[related]
    errors: Vec<VerificationFailure>,
}

#[derive(Debug, Error, Diagnostic)]
#[error("skipping calculation {calculation_index}, artifact {name} ({representation}): {reason}")]
#[diagnostic(code(rustiq::artifact::verification_skipped), severity(Warning))]
struct VerificationWarning {
    calculation_index: usize,
    name: String,
    representation: String,
    reason: &'static str,
    #[help]
    help: &'static str,
}

#[derive(Serialize)]
struct VerificationReport {
    schema_version: u32,
    kind: &'static str,
    status: &'static str,
    calculation_count: usize,
    verified: usize,
    failed: usize,
    skipped: usize,
    artifacts: Vec<VerificationEntry>,
}

#[derive(Serialize)]
struct VerificationEntry {
    calculation_index: usize,
    name: String,
    representation: String,
    status: &'static str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    errors: Vec<String>,
}

impl VerificationReport {
    fn write_text(&self, mut output: impl Write) -> io::Result<()> {
        writeln!(
            output,
            "Verification {}\n  Calculations: {}\n  Verified:     {}\n  Failed:       {}\n  Skipped:      {}",
            self.status, self.calculation_count, self.verified, self.failed, self.skipped
        )?;
        if self.artifacts.is_empty() {
            writeln!(
                output,
                "  No numerical artifacts declared; no numerical payload was verified."
            )?;
        }
        for entry in &self.artifacts {
            let status = match entry.status {
                "valid" => "Valid",
                "invalid" => "Invalid",
                "unknown_artifact" => "Unknown artifact",
                "unsupported_representation" => "Unsupported representation",
                _ => "Unchecked",
            };
            writeln!(
                output,
                "  Calculation {}, artifact {} ({}): {status}",
                entry.calculation_index, entry.name, entry.representation
            )?;
        }
        Ok(())
    }
}

impl Runnable for VerifyCommand {
    fn run(&self) -> CommandResult {
        let summary = RustiQBundle::check_archive(&self.path).into_diagnostic()?;
        let json = matches!(self.format, CalculationOutputFormat::Json);
        let mut report = VerificationReport {
            schema_version: 1,
            kind: "artifact-verification",
            status: "successful",
            calculation_count: summary.calculation_count,
            verified: 0,
            failed: 0,
            skipped: 0,
            artifacts: Vec::with_capacity(summary.artifacts.len()),
        };
        let mut errors = Vec::new();
        for artifact in summary.artifacts {
            use ArtifactCheckStatus::{Invalid, UnknownArtifact, UnsupportedRepresentation, Valid};
            let mut entry = VerificationEntry {
                calculation_index: artifact.calculation_index,
                name: artifact.name,
                representation: artifact.representation,
                status: "valid",
                errors: Vec::new(),
            };
            match artifact.status {
                Valid => report.verified += 1,
                Invalid(error) => {
                    entry.status = "invalid";
                    let source = miette::Report::from_err(error);
                    if json {
                        entry.errors = source.chain().map(ToString::to_string).collect();
                    }
                    errors.push(VerificationFailure {
                        calculation_index: entry.calculation_index,
                        name: entry.name.clone(),
                        representation: entry.representation.clone(),
                        source: source.into(),
                    });
                }
                status => {
                    let (label, reason, help) = match status {
                        UnknownArtifact => (
                            "unknown_artifact",
                            "unknown artifact",
                            "This RustiQ version does not recognize the artifact name. Its payload was not checked.",
                        ),
                        UnsupportedRepresentation => (
                            "unsupported_representation",
                            "unsupported representation",
                            "This RustiQ version cannot read this representation. Its payload was not checked; an unsupported representation is not evidence of corruption.",
                        ),
                        _ => ("unchecked", "unsupported check outcome", "Its payload was not checked."),
                    };
                    entry.status = label;
                    report.skipped += 1;
                    crate::cli::ux::diagnostics::emit_warning(VerificationWarning {
                        calculation_index: entry.calculation_index,
                        name: entry.name.clone(),
                        representation: entry.representation.clone(),
                        reason,
                        help,
                    })?;
                }
            }
            report.artifacts.push(entry);
        }

        report.failed = errors.len();
        report.status = if report.failed > 0 {
            "failed"
        } else if report.skipped > 0 {
            "incomplete"
        } else {
            "successful"
        };
        let mut stdout = io::stdout().lock();
        if json {
            serde_json::to_writer(&mut stdout, &report).into_diagnostic()?;
            writeln!(stdout).into_diagnostic()?;
        } else {
            report.write_text(&mut stdout).into_diagnostic()?;
        }
        if !errors.is_empty() {
            Err(VerificationFailures { errors }.into())
        } else if self.strict && report.skipped > 0 {
            Err(crate::cli::exit_status::IncompleteVerification {
                skipped: report.skipped,
            }
            .into())
        } else {
            Ok(())
        }
    }
}

impl InspectCommand {
    fn inspect(&self) -> miette::Result<Inspection> {
        let bundle = RustiQBundle::open(&self.path).into_diagnostic()?;
        let sources = bundle
            .sources()
            .map(|source| Source {
                original_name: source.original_name().to_owned(),
                size: source.bytes().len(),
            })
            .collect();
        let calculations = bundle
            .into_calculations()
            .into_iter()
            .enumerate()
            .map(|(source_index, data)| {
                let (name, version) = data.producer();
                let producer = Producer {
                    name: name.to_owned(),
                    version: version.to_owned(),
                };
                let artifacts = data
                    .artifact_representations()
                    .map(|(name, representation)| Representation {
                        name: name.to_owned(),
                        representation: representation.to_owned(),
                    })
                    .collect();
                let request = data
                    .request()
                    .ok_or_else(|| miette!("missing portable request"))?;
                let requested = portable_requested_calculation(request).into_diagnostic()?;
                let prepared = data.prepare_calculation().into_diagnostic()?;
                let resolved = resolved_calculation(&prepared).into_diagnostic()?;
                Ok(Entry {
                    source_index,
                    producer,
                    artifacts,
                    requested: Canonical {
                        configuration: requested.toml,
                        geometry: requested.xyz,
                    },
                    resolved: Canonical {
                        configuration: resolved.configuration,
                        geometry: resolved.geometry,
                    },
                })
            })
            .collect::<miette::Result<Vec<_>>>()?;
        let inspection = Inspection {
            schema_version: 1,
            kind: "artifact-inspection",
            payloads_verified: false,
            sources,
            calculations,
        };
        Ok(inspection)
    }
}

impl Runnable for InspectCommand {
    fn run(&self) -> CommandResult {
        let inspection = self.inspect()?;
        if self.format == CalculationOutputFormat::Json {
            serde_json::to_writer(io::stdout().lock(), &inspection).into_diagnostic()?;
            writeln!(io::stdout().lock()).into_diagnostic()?;
        } else {
            writeln!(
                io::stdout().lock(),
                "Portable RustiQ V1: {} calculations",
                inspection.calculations.len()
            )
            .into_diagnostic()?;
            writeln!(
                io::stdout().lock(),
                "Numeric payloads have not been fully verified."
            )
            .into_diagnostic()?;
            for source in &inspection.sources {
                writeln!(
                    io::stdout().lock(),
                    "Source: {} ({} bytes)",
                    source.original_name,
                    source.size
                )
                .into_diagnostic()?;
            }
            for entry in &inspection.calculations {
                writeln!(
                    io::stdout().lock(),
                    "\nCalculation index {} ({} {})",
                    entry.source_index,
                    entry.producer.name,
                    entry.producer.version
                )
                .into_diagnostic()?;
                writeln!(
                    io::stdout().lock(),
                    "{}",
                    crate::cli::color::title("Requested calculation (canonical TOML)")
                )
                .into_diagnostic()?;
                bat::print_toml(&entry.requested.configuration);
                writeln!(
                    io::stdout().lock(),
                    "{}",
                    crate::cli::color::title("Requested geometry (canonical XYZ)")
                )
                .into_diagnostic()?;
                bat::print_xyz(&entry.requested.geometry);
                writeln!(
                    io::stdout().lock(),
                    "{}",
                    crate::cli::color::title("Resolved calculation (canonical TOML)")
                )
                .into_diagnostic()?;
                bat::print_toml(&entry.resolved.configuration);
                writeln!(
                    io::stdout().lock(),
                    "{}",
                    crate::cli::color::title("Resolved geometry (Bohr)")
                )
                .into_diagnostic()?;
                bat::print_xyz(&entry.resolved.geometry);
                for artifact in &entry.artifacts {
                    writeln!(
                        io::stdout().lock(),
                        "Artifact {}: {}",
                        artifact.name,
                        artifact.representation
                    )
                    .into_diagnostic()?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod verification_tests {
    use super::*;

    #[test]
    fn diagnostics_preserve_severity_context_and_sources() {
        let warning = VerificationWarning {
            calculation_index: 2,
            name: "future".into(),
            representation: "v2".into(),
            reason: "unknown artifact",
            help: "The payload was not checked.",
        };
        assert_eq!(warning.severity(), Some(miette::Severity::Warning));
        assert_eq!(
            warning.code().unwrap().to_string(),
            "rustiq::artifact::verification_skipped"
        );
        let failure = VerificationFailure {
            calculation_index: 1,
            name: "ao_eri".into(),
            representation: "rustiq-compact-eri-v1".into(),
            source: miette!("invalid payload").into(),
        };
        assert!(std::error::Error::source(&failure).is_some());
        assert!(failure.diagnostic_source().is_some());
        let batch = VerificationFailures {
            errors: vec![failure],
        };
        assert_eq!(batch.related().unwrap().count(), 1);
    }
}
