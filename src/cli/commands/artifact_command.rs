use std::{
    io::{self, Write},
    path::PathBuf,
};

use clap::Subcommand;
use miette::{miette, IntoDiagnostic};
use rustiq_core::persistence::RustiQBundle;
use serde::Serialize;

use super::{run_command::CalculationOutputFormat, CommandResult, Runnable};
use crate::cli::ux::calculation_presentation::{
    portable_requested_calculation, resolved_calculation,
};

#[derive(Debug, Subcommand)]
pub(crate) enum ArtifactCommands {
    /// Inspect versioned requested/resolved snapshots without executing calculations.
    Inspect(InspectCommand),
}

#[derive(Debug, clap::Args)]
pub(crate) struct InspectCommand {
    path: PathBuf,
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
        let mut stdout = io::stdout().lock();
        if self.format == CalculationOutputFormat::Json {
            serde_json::to_writer(&mut stdout, &inspection).into_diagnostic()?;
            writeln!(stdout).into_diagnostic()?;
        } else {
            writeln!(
                stdout,
                "Portable RustiQ V1: {} calculations",
                inspection.calculations.len()
            )
            .into_diagnostic()?;
            writeln!(stdout, "Numeric payloads have not been fully verified.").into_diagnostic()?;
            for source in &inspection.sources {
                writeln!(
                    stdout,
                    "Source: {} ({} bytes)",
                    source.original_name, source.size
                )
                .into_diagnostic()?;
            }
            for entry in &inspection.calculations {
                writeln!(
                    stdout,
                    "\nCalculation index {} ({} {})",
                    entry.source_index, entry.producer.name, entry.producer.version
                )
                .into_diagnostic()?;
                writeln!(
                    stdout,
                    "Requested calculation (canonical TOML):\n{}\nRequested geometry:\n{}",
                    entry.requested.configuration, entry.requested.geometry
                )
                .into_diagnostic()?;
                writeln!(
                    stdout,
                    "Resolved calculation (canonical TOML):\n{}\nResolved geometry (Bohr):\n{}",
                    entry.resolved.configuration, entry.resolved.geometry
                )
                .into_diagnostic()?;
                for artifact in &entry.artifacts {
                    writeln!(
                        stdout,
                        "Artifact {}: {}",
                        artifact.name, artifact.representation
                    )
                    .into_diagnostic()?;
                }
            }
        }
        Ok(())
    }
}
