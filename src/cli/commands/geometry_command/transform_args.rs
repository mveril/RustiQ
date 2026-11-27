use std::{
    fs,
    io::{self, stdin},
    path::{Path, PathBuf},
};

use clap::Args;
use miette::{miette, IntoDiagnostic};

use rustiq_core::molecules::Geometry;

#[derive(Args, Debug, Clone)]
pub struct TransformArgs {
    /// XYZ geometry file to read. Reads from standard input when omitted.
    #[clap(default_value = None)]
    pub input: Option<PathBuf>,
    /// XYZ geometry file to write. Writes to standard output when omitted.
    #[clap(default_value = None, short = 'o', long)]
    pub output: Option<PathBuf>,
    #[command(flatten)]
    pub(crate) output_options: super::super::OutputOptions,
}

impl TransformArgs {
    pub fn apply_transform(
        &self,
        f: impl FnOnce(&mut Geometry) -> miette::Result<()>,
    ) -> miette::Result<()> {
        let mut geometry = if let Some(input) = &self.input {
            Geometry::from_path(input)?
        } else {
            Geometry::from_reader(stdin().lock())?
        };
        f(&mut geometry)?;
        if let Some(output) = &self.output {
            // Resolve an existing alias before replacing its target, not the link.
            let output = match fs::canonicalize(output) {
                Ok(path) => path,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    match fs::symlink_metadata(output) {
                        Ok(_) => {
                            return Err(miette!(
                                code = "rustiq::geometry::dangling_output",
                                "Geometry output {} is a dangling symbolic link",
                                output.display()
                            ));
                        }
                        Err(error) if error.kind() == io::ErrorKind::NotFound => output.clone(),
                        Err(error) => return Err(error).into_diagnostic(),
                    }
                }
                Err(error) => return Err(error).into_diagnostic(),
            };
            if !self.output_options.force && output.exists() {
                return Err(miette!(
                    code = "rustiq::geometry::output_exists",
                    "Geometry output {} already exists; use --force to replace it",
                    output.display()
                ));
            }
            let parent = output
                .parent()
                .filter(|path| !path.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            let mut temporary = tempfile::NamedTempFile::new_in(parent).into_diagnostic()?;
            geometry
                .to_writer(temporary.as_file_mut())
                .into_diagnostic()?;
            let publication = if self.output_options.force {
                temporary.persist(&output)
            } else {
                temporary.persist_noclobber(&output)
            };
            publication.map_err(|error| {
                if error.error.kind() == io::ErrorKind::AlreadyExists {
                    miette!(
                        code = "rustiq::geometry::output_exists",
                        "Geometry output {} already exists; use --force to replace it",
                        output.display()
                    )
                } else {
                    miette!(
                        code = "rustiq::geometry::publish_failed",
                        "could not publish geometry {}: {}",
                        output.display(),
                        error.error
                    )
                }
            })?;
        } else {
            geometry.to_writer(io::stdout()).into_diagnostic()?;
        }
        Ok(())
    }
}
