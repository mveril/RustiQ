use std::{
    env,
    fs::{self, File},
    io::{self, Read},
    path::PathBuf,
};

use crate::{
    basis::{basis_store::BasisStore, basisfile::BasisFile, gaussian::basis::Basis},
    hf,
    molecules::{geometry::Geometry, molecule::Molecule},
    runfile::RunFile,
};

use super::Runnable;

#[derive(clap::Args, Debug)] // Permet d'utiliser cette structure avec Clap
pub struct RunCommand {
    /// The toml file used for the calculation. If not specified, the standard input is used.
    #[arg(short, long)]
    pub file: Option<PathBuf>,
}

impl Runnable for RunCommand {
    fn run(&self) {
        let toml_content = if let Some(path_toml) = &self.file {
            let content = fs::read_to_string(path_toml).unwrap();
            let dir = path_toml.parent().unwrap();
            env::set_current_dir(dir).unwrap();
            content
        } else {
            let mut content = String::new();
            io::stdin().read_to_string(&mut content).unwrap();
            content
        };
        let run = toml::from_str::<RunFile>(&toml_content).unwrap();
        print!("{}", toml::to_string(&run).unwrap());
        let molecule_path = &run.global.molecule.geometry;
        let molfile = File::open(molecule_path).unwrap();
        let geom = Geometry::from_file(molfile, None, None).unwrap();
        let molecule = Molecule {
            geometry: geom,
            charge: run.global.molecule.charge,
            multiplicity: run.global.molecule.multiplicity,
        };
        println!("{}", &molecule.geometry);
        let store = BasisStore::default();
        let basis_file: BasisFile = store.get(&run.global.basis).unwrap();
        println!("{} {:?}", basis_file.name, basis_file.function_types);
        let basis = Basis::load(&basis_file, &molecule);
        if let Some(hf) = run.hf {
            println!("Conv {}", hf.convergence_threshold);
            println!("Max iter: {}", hf.max_iterations);
            let mut scf = hf::scf::ScfCalculation::new(
                &molecule,
                &basis,
                hf.max_iterations,
                hf.convergence_threshold,
                hf.density_guess.get_density_guess(),
            );
            scf.run();
        }
    }
}
