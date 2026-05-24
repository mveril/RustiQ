use std::{collections::BTreeMap, fs::File, io::stdin};

use crate::{
    cli::commands::{CommandResult, Runnable},
    molecules::geometry::Geometry,
};
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
            Some(path) => {
                Geometry::from_reader(std::io::BufReader::new(File::open(path)?), None, None)
            }
            None => Geometry::from_reader(std::io::BufReader::new(stdin().lock()), None, None),
        }?;
        println!("Number of atoms: {}", &geometry.atoms.len());
        println!("Nuclear repulsion energy: {}", &geometry.nucl_repulsion());
        println!("Center of mass: {}", &geometry.mass_center()?);
        println!("Center of charge: {}", &geometry.charge_center());
        println!("Center {}", &geometry.center());
        let mut counts = BTreeMap::new();
        for atom in &geometry.atoms {
            *counts.entry(atom.element.symbol).or_insert(0) += 1;
        }
        for (element, count) in counts {
            println!("{}: {}", element, count);
        }

        Ok(())
    }
}
