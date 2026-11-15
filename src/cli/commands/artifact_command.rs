use std::{
    io::{self, Write},
    path::PathBuf,
};

use clap::Subcommand;
use miette::{miette, Diagnostic, IntoDiagnostic};
use rustiq_core::persistence::{
    AoEriArtifact, Artifact, ConvergedHfArtifact, RustiQBundle, RustiQData,
};
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
}

fn verify_payload<A: Artifact>(data: &RustiQData) -> miette::Result<()> {
    data.get::<A>()
        .into_diagnostic()?
        .ok_or_else(|| miette!("declared artifact payload is missing"))?;
    Ok(())
}

impl Runnable for VerifyCommand {
    fn run(&self) -> CommandResult {
        let bundle = RustiQBundle::open(&self.path).into_diagnostic()?;
        let calculation_count = bundle.calculations().len();
        let mut verified = 0;
        let mut skipped = 0;
        let mut errors = Vec::new();

        for (calculation_index, data) in bundle.into_calculations().into_iter().enumerate() {
            for (name, representation) in data.artifact_representations() {
                let reason = match name {
                    "ao_eri" if representation == "rustiq-compact-eri-v1" => None,
                    "converged_hf"
                        if matches!(
                            representation,
                            "rustiq-converged-hf-orbitals-v1"
                                | "rustiq-converged-hf-npy-v1"
                                | "rustiq-converged-hf-v1"
                        ) =>
                    {
                        None
                    }
                    "ao_eri" | "converged_hf" => Some("unsupported representation"),
                    _ => Some("unknown artifact"),
                };
                if let Some(reason) = reason {
                    skipped += 1;
                    let warning = miette::Report::new(VerificationWarning {
                        calculation_index,
                        name: name.to_owned(),
                        representation: representation.to_owned(),
                        reason,
                    });
                    writeln!(io::stderr().lock(), "{warning:?}").into_diagnostic()?;
                    continue;
                }

                let result = match name {
                    "ao_eri" => verify_payload::<AoEriArtifact>(&data),
                    _ => verify_payload::<ConvergedHfArtifact>(&data),
                };
                match result {
                    Ok(()) => verified += 1,
                    Err(source) => errors.push(VerificationFailure {
                        calculation_index,
                        name: name.to_owned(),
                        representation: representation.to_owned(),
                        source: source.into(),
                    }),
                }
            }
        }
        writeln!(io::stdout().lock(),
            "Verification summary\n  Calculations: {calculation_count}\n  Verified:     {verified}\n  Failed:       {}\n  Skipped:      {skipped}", errors.len()
        ).into_diagnostic()?;
        if errors.is_empty() {
            Ok(())
        } else {
            Err(VerificationFailures { errors }.into())
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
