use std::{
    env,
    fs::{self, File},
    io::{self, Read},
    path::PathBuf,
};

use crate::{
    basis::{basis_store::BasisStore, basisfile::BasisFile, gaussian::basis::Basis},
    cli::ux::scf_report::ScfReporter,
    hf,
    molecules::{geometry::Geometry, molecule::Molecule, units::Units},
    runfile::hf::HfOutputFormat,
    runfile::RunFile,
};

use super::{CommandResult, Runnable};

#[derive(clap::Args, Debug)] // Allows this structure to be used with Clap
pub struct RunCommand {
    /// The toml file used for the calculation. If not specified, the standard input is used.
    #[arg(short, long)]
    pub file: Option<PathBuf>,
}

impl Runnable for RunCommand {
    fn run(&self) -> CommandResult {
        let toml_content = if let Some(path_toml) = &self.file {
            let content = fs::read_to_string(path_toml)?;
            if let Some(dir) = path_toml.parent() {
                env::set_current_dir(dir)?;
            }
            content
        } else {
            let mut content = String::new();
            io::stdin().read_to_string(&mut content)?;
            content
        };
        let run = toml::from_str::<RunFile>(&toml_content)?;
        print!("{}", toml::to_string(&run)?);
        let molecule_path = &run.global.molecule.geometry;
        let molfile = File::open(molecule_path)?;
        let geom = Geometry::from_file(molfile)?;
        let mut molecule = Molecule::new(
            geom,
            run.global.molecule.molecule_unit,
            run.global.molecule.charge,
            run.global.molecule.multiplicity,
        );
        println!("{}", &molecule.geometry);
        molecule.convert_to(Units::Bohr);
        let store = BasisStore::default();
        let basis_file: BasisFile = store.get(&run.global.basis)?;
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
            if hf.diis {
                scf.enable_diis(hf.diis_size)?;
            }
            match hf.format {
                HfOutputFormat::Normal => {
                    let stdout = io::stdout();
                    let mut reporter = ScfReporter::new(stdout.lock());
                    let result = scf.run_with_observer(&mut reporter);
                    if let Some(err) = reporter.take_error() {
                        return Err(err.into());
                    }
                    reporter.write_summary(&result)?;
                }
                HfOutputFormat::Nope => {
                    scf.run();
                }
            }
        }
        Ok(())
    }
}
