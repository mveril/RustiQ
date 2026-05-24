use anyhow::Result;
use clap::Args;
use std::{fs::File, io::stdin, path::PathBuf};

use crate::molecules::geometry::Geometry;

#[derive(Args, Debug, Clone)]
pub struct TransformArgs {
    /// XYZ geometry file to read. Reads from standard input when omitted.
    #[clap(default_value = None)]
    pub input: Option<PathBuf>,
    /// XYZ geometry file to write. Writes to standard output when omitted.
    #[clap(default_value = None, short = 'o', long)]
    pub output: Option<PathBuf>,
}

impl TransformArgs {
    pub fn apply_transform(&self, f: impl FnOnce(&mut Geometry) -> Result<()>) -> Result<()> {
        let mut geometry = if let Some(input) = &self.input {
            Geometry::from_file(File::open(input)?, None, None)?
        } else {
            Geometry::from_reader(stdin().lock(), None, None)?
        };
        f(&mut geometry)?;
        if let Some(output) = &self.output {
            geometry.to_writer(File::create(output)?)?;
        } else {
            geometry.to_writer(std::io::stdout())?;
        }
        Ok(())
    }
}
