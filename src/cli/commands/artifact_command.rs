use std::{
    io::{self, Write},
    path::PathBuf,
};

use clap::Subcommand;
use miette::{miette, IntoDiagnostic};
use rustiq_core::persistence::{AoEriArtifact, Artifact, RustiQBundle};
use serde::Serialize;

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

impl Runnable for VerifyCommand {
    fn run(&self) -> CommandResult {
        let bundle = RustiQBundle::open(&self.path).into_diagnostic()?;
        let calculation_count = bundle.calculations().len();
        let mut verified = 0;

        for (calculation_index, data) in bundle.into_calculations().into_iter().enumerate() {
            let artifacts = data
                .artifact_representations()
                .map(|(name, representation)| (name.to_owned(), representation.to_owned()))
                .collect::<Vec<_>>();
            let mut data = data;
            for (name, representation) in artifacts {
                match name.as_str() {
                    name if name == AoEriArtifact::NAME
                        && representation == "rustiq-compact-eri-v1" =>
                    {
                        data.get::<AoEriArtifact>()
                            .into_diagnostic()?
                            .ok_or_else(|| miette!("AO ERI payload is missing"))?;
                        verified += 1;
                    }
                    _ => {
                        return Err(miette!(
                            "cannot verify unsupported artifact {name} ({representation}) in calculation {calculation_index}"
                        ));
                    }
                }
            }
        }

        writeln!(
            io::stdout().lock(),
            "Verified {verified} payload(s) across {calculation_count} calculation(s)."
        )
        .into_diagnostic()?;
        Ok(())
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
