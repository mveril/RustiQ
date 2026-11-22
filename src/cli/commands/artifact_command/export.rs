use std::{
    fs, io,
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
    #[command(flatten)]
    output_options: super::super::OutputOptions,
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
            write_export_file(&self.path, path, &export, self.output_options.force)
                .wrap_err_with(|| format!("could not write export file {}", path.display()))
        } else {
            export.write_to(io::stdout().lock()).into_diagnostic()
        }
    }
}

fn write_export_file(
    input: &Path,
    output: &Path,
    export: &rustiq_core::persistence::ArtifactExport,
    force: bool,
) -> miette::Result<()> {
    // Follow existing symlinks, preserving their target and checking aliases.
    let output = match fs::canonicalize(output) {
        Ok(path) => {
            if same_file::is_same_file(input, &path).into_diagnostic()? {
                return Err(miette!("Export output must not replace the input bundle."));
            }
            path
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => output.to_path_buf(),
        Err(error) => return Err(error).into_diagnostic(),
    };
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !force && output.exists() {
        return Err(miette!(
            "Export output {} already exists. Use --force to replace it.",
            output.display()
        ));
    }
    let mut file = tempfile::NamedTempFile::new_in(parent).into_diagnostic()?;
    export.write_to(file.as_file_mut()).into_diagnostic()?;
    if force {
        file.persist(&output).into_diagnostic()?;
    } else {
        file.persist_noclobber(&output).map_err(|error| {
            if error.error.kind() == io::ErrorKind::AlreadyExists {
                miette!(
                    "Export output {} already exists. Use --force to replace it.",
                    output.display()
                )
            } else {
                miette!("could not publish export file: {}", error.error)
            }
        })?;
    }
    Ok(())
}
