use clap::Args;
use miette::IntoDiagnostic;
use std::{fs::OpenOptions, io::stdin, path::PathBuf};

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
            geometry
                .to_writer(
                    OpenOptions::new()
                        .write(true)
                        .create(true)
                        .create_new(!self.output_options.force)
                        .truncate(self.output_options.force)
                        .open(output)
                        .into_diagnostic()?,
                )
                .into_diagnostic()?;
        } else {
            geometry.to_writer(std::io::stdout()).into_diagnostic()?;
        }
        Ok(())
    }
}
