use std::{
    fs::OpenOptions,
    io,
    path::{Path, PathBuf},
};

use clap::{Args, ValueEnum};
use miette::{miette, IntoDiagnostic, WrapErr};
use rustiq_core::persistence::{ExportError, ExportFormat, RustiQBundle};

use crate::cli::commands::{CommandResult, Runnable};

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CliExportFormat {
    Molden,
}

#[derive(Debug, Args)]
pub(crate) struct ExportCommand {
    /// Portable .rustiq bundle.
    path: PathBuf,
    /// Select the export format.
    #[arg(long, value_enum)]
    format: CliExportFormat,
    /// Zero-based calculation index. Required for bundles with multiple calculations.
    #[arg(long)]
    calculation: Option<usize>,
    /// File to write. Writes to standard output when omitted.
    #[arg(short, long)]
    output: Option<PathBuf>,
}

impl Runnable for ExportCommand {
    fn run(&self) -> CommandResult {
        let bundle = RustiQBundle::open_for_restore(&self.path).into_diagnostic()?;
        let calculations = bundle.calculations();
        let index = match self.calculation {
            Some(index) => index,
            None if calculations.len() == 1 => 0,
            None => {
                return Err(miette!(
                    "This bundle contains {} calculations. Use --calculation with a zero-based index.",
                    calculations.len()
                ));
            }
        };
        let data = calculations.get(index).ok_or_else(|| {
            miette!(
                "Calculation index {index} is out of range. Use a zero-based index from 0 to {}.",
                calculations.len() - 1
            )
        })?;
        let format = match self.format {
            CliExportFormat::Molden => ExportFormat::Molden,
        };
        let export = data.export(format).map_err(|error| match error {
            ExportError::MissingOrbitals => miette!(
                "Calculation {index} has no saved converged HF orbitals. Save a converged HF calculation with rustiq run --artifact to export its orbitals."
            ),
            ExportError::Artifact(source) => miette::Report::from_err(source).wrap_err(
                format!("could not read the converged HF artifact for calculation {index}"),
            ),
            error => miette::Report::from_err(error)
                .wrap_err(format!("could not export calculation {index}")),
        })?;

        if let Some(path) = &self.output {
            let file = open_export_file(&self.path, path)?;
            export
                .write_to(file.as_file())
                .into_diagnostic()
                .wrap_err_with(|| format!("could not write export file {}", path.display()))
        } else {
            export.write_to(io::stdout().lock()).into_diagnostic()
        }
    }
}

fn open_export_file(input: &Path, output: &Path) -> miette::Result<same_file::Handle> {
    let input_file = same_file::Handle::from_path(input).into_diagnostic()?;
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(output)
        .and_then(same_file::Handle::from_file)
        .into_diagnostic()
        .wrap_err_with(|| format!("could not open export output {}", output.display()))?;
    // Compare the open handles before truncating, including symlinks and hard links.
    if file == input_file {
        return Err(miette!("Export output must not replace the input bundle."));
    }
    file.as_file()
        .set_len(0)
        .into_diagnostic()
        .wrap_err_with(|| format!("could not truncate export output {}", output.display()))?;
    Ok(file)
}
