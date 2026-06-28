use std::{
    fs::File,
    path::{Path, PathBuf},
};

use miette::{Context, Diagnostic, IntoDiagnostic};
use thiserror::Error;

use crate::{
    basis::BasisStore,
    cli::commands::{CommandResult, Runnable},
};

#[derive(clap::Args, Debug)]
pub struct ImportCommand {
    /// Paths to one or more basis set JSON files
    #[arg(value_name = "PATH", required = true)]
    pub paths: Vec<PathBuf>,
}

#[derive(Debug, Error, Diagnostic)]
#[error("failed to import basis set file `{path}`")]
#[diagnostic(code(rustiq::basis::import_file_failed))]
struct ImportFileError {
    path: PathBuf,

    #[source]
    #[diagnostic_source]
    source: Box<dyn Diagnostic + Send + Sync + 'static>,
}

#[derive(Debug, Error, Diagnostic)]
#[error("{failed} basis set import(s) failed")]
#[diagnostic(
    code(rustiq::basis::import_failed),
    help("Some files may have been imported successfully. Fix the listed files and retry.")
)]
struct ImportBatchError {
    succeeded: usize,
    failed: usize,

    #[related]
    failures: Vec<ImportFileError>,
}

impl ImportCommand {
    fn import_one(store: &BasisStore, path: &Path) -> miette::Result<String> {
        let file = File::open(path)
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to open basis set file: {}", path.display()))?;

        store
            .import(file)
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to import basis set file: {}", path.display()))
    }
}

impl Runnable for ImportCommand {
    fn run(&self) -> CommandResult {
        let store = BasisStore::default();

        let mut succeeded = 0;
        let mut failures = Vec::new();

        for path in &self.paths {
            match Self::import_one(&store, path) {
                Ok(name) => {
                    succeeded += 1;
                    println!("Basis {name} imported.");
                }
                Err(error) => {
                    failures.push(ImportFileError {
                        path: path.clone(),
                        source: error.into(),
                    });
                }
            }
        }

        if failures.is_empty() {
            return Ok(());
        }

        let failed = failures.len();

        Err(ImportBatchError {
            succeeded,
            failed,
            failures,
        }
        .into())
    }
}
