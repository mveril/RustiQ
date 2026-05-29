use std::{
    env,
    fs::{self, File},
    io::{self, Read},
    path::PathBuf,
    time::Instant,
};

use crate::{
    basis::{basis_store::BasisStore, basisfile::BasisFile, gaussian::basis::Basis},
    cli::ux::{bat, scf_report::ScfReporter},
    hf,
    molecules::{geometry::Geometry, molecule::Molecule, units::Units},
    runfile::hf::HfOutputFormat,
    runfile::RunFile,
};

use super::{CommandResult, Runnable};

#[derive(clap::Args, Debug)] // Allows this structure to be used with Clap
pub struct RunCommand {
    /// The toml file used for the calculation. If not specified, the standard input is used.
    pub input: Option<PathBuf>,
}

impl Runnable for RunCommand {
    fn run(&self) -> CommandResult {
        let toml_content = if let Some(path_toml) = &self.input {
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
        bat::print_toml(&toml_content);
        let molecule_path = &run.global.molecule.geometry;
        let molfile = File::open(molecule_path)?;
        let geom = Geometry::from_file(molfile)?;
        let mut molecule = Molecule::new(
            geom,
            run.global.molecule.molecule_unit,
            run.global.molecule.charge,
            run.global.molecule.multiplicity,
        );
        bat::print_xyz(&molecule.geometry.to_string());
        molecule.convert_to(Units::Bohr);
        let store = BasisStore::default();
        println!("Loading basis set...");
        let step_start = Instant::now();
        let basis_file: BasisFile = store.get(&run.global.basis)?;
        println!("{} {:?}", basis_file.name, basis_file.function_types);
        println!(
            "Basis file loaded in {}",
            humantime::format_duration(step_start.elapsed())
        );
        println!("Constructing basis functions...");
        let step_start = Instant::now();
        let basis = Basis::load(&basis_file, &molecule);
        println!(
            "Constructed {} basis functions in {}",
            basis.nbasis(),
            humantime::format_duration(step_start.elapsed())
        );
        if let Some(hf) = run.hf {
            println!("Conv {}", hf.convergence_threshold);
            println!("Max iter: {}", hf.max_iterations);
            println!("Preparing SCF calculation...");
            let mut scf = hf::scf::ScfCalculation::new_with_progress(
                &molecule,
                &basis,
                hf.max_iterations,
                hf.convergence_threshold,
                hf.guess,
                |step| println!("  {step}..."),
            )?;
            if hf.diis {
                scf.enable_diis(hf.diis_size)?;
            }
            match hf.format {
                HfOutputFormat::Normal => {
                    let stdout = io::stdout();
                    let mut reporter = ScfReporter::new(stdout.lock());
                    let result = scf.run_with_observer(&mut reporter)?;
                    if let Some(err) = reporter.take_error() {
                        return Err(err.into());
                    }
                    reporter.write_summary(&result)?;
                }
                HfOutputFormat::Nope => {
                    scf.run()?;
                }
            }
        }
        Ok(())
    }
}
