use std::{
    collections::BTreeMap,
    io::{stdin, Write},
    path::PathBuf,
};

use miette::IntoDiagnostic;

use crate::cli::{
    color,
    commands::{CommandResult, Runnable},
};
use rustiq_core::molecules::geometry::Geometry;

#[derive(clap::Args, Debug)]
pub struct InfoCommand {
    /// Molecular geometry file to read
    /// Supported format: XYZ
    pub path: Option<PathBuf>,
}

impl Runnable for InfoCommand {
    fn run(&self) -> CommandResult {
        let geometry = match &self.path {
            Some(path) => Geometry::from_path(path),
            None => Geometry::from_reader(std::io::BufReader::new(stdin().lock())),
        }?;
        let mut stdout = color::stdout().lock();
        writeln!(
            stdout,
            "{} {}",
            color::title("Number of atoms:"),
            geometry.atoms.len()
        )
        .into_diagnostic()?;
        writeln!(
            stdout,
            "{} {}",
            color::title("Nuclear repulsion energy:"),
            color::value(geometry.nucl_repulsion())
        )
        .into_diagnostic()?;
        writeln!(
            stdout,
            "{} {}",
            color::title("Center of mass:"),
            geometry.mass_center().into_diagnostic()?
        )
        .into_diagnostic()?;
        writeln!(
            stdout,
            "{} {}",
            color::title("Center of charge:"),
            geometry.charge_center()
        )
        .into_diagnostic()?;
        writeln!(stdout, "{} {}", color::title("Center"), geometry.center()).into_diagnostic()?;

        let mut counts = BTreeMap::new();
        for atom in &geometry.atoms {
            *counts.entry(atom.element.symbol).or_insert(0) += 1;
        }
        for (element, count) in counts {
            writeln!(stdout, "{}: {}", color::value(element), count).into_diagnostic()?;
        }

        Ok(())
    }
}
