use std::{collections::BTreeMap, io::stdin};

use miette::IntoDiagnostic;

use crate::cli::commands::{CommandResult, Runnable};
use rustiq_core::molecules::geometry::Geometry;
use std::path::PathBuf;

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
        println!(
            "{} {}",
            crate::cli::color::title("Number of atoms:"),
            geometry.atoms.len()
        );
        println!(
            "{} {}",
            crate::cli::color::title("Nuclear repulsion energy:"),
            crate::cli::color::value(geometry.nucl_repulsion())
        );
        println!(
            "{} {}",
            crate::cli::color::title("Center of mass:"),
            geometry.mass_center().into_diagnostic()?
        );
        println!(
            "{} {}",
            crate::cli::color::title("Center of charge:"),
            geometry.charge_center()
        );
        println!(
            "{} {}",
            crate::cli::color::title("Center"),
            geometry.center()
        );
        let mut counts = BTreeMap::new();
        for atom in &geometry.atoms {
            *counts.entry(atom.element.symbol).or_insert(0) += 1;
        }
        for (element, count) in counts {
            println!("{}: {}", crate::cli::color::value(element), count);
        }

        Ok(())
    }
}
