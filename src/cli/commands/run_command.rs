use std::{
    env, fs,
    io::{self, Read},
    path::PathBuf,
    time::Instant,
};

use clap::ArgAction;
use miette::{miette, Diagnostic, IntoDiagnostic};
use thiserror::Error;

use crate::{
    basis::{gaussian::basis::Basis, BasisFile, BasisStore},
    cli::{
        self,
        ux::{bat, mp2_report::Mp2Reporter, scf_report::ScfReporter},
    },
    hf::{self, scf_result::ScfResult},
    molecules::{geometry::Geometry, molecule::Molecule, units::Units},
    mp2 as mp2_calc,
    runfile::{
        hf::{HfOutputFormat, ResolvedHfMethod},
        parser::parse_runfile,
    },
};

use super::{CommandResult, Runnable};

#[derive(Debug, Error, Diagnostic)]
enum MoleculeConfigError {
    #[error("invalid molecule: total electron count must be positive, got {electrons}")]
    #[diagnostic(
        code(rustiq::runfile::molecule_electron_count),
        help("Check the molecule geometry and charge in the runfile.")
    )]
    NonPositiveElectronCount { electrons: i32 },

    #[error(
        "invalid molecule: total electrons = {electrons}, multiplicity = {multiplicity} are incompatible"
    )]
    #[diagnostic(
        code(rustiq::runfile::molecule_multiplicity),
        help("For a valid electron configuration, the requested spin state must be compatible with the electron count.")
    )]
    IncompatibleMultiplicity { electrons: i32, multiplicity: u8 },
}

fn validate_molecule_config(molecule: &Molecule) -> Result<(), MoleculeConfigError> {
    let electrons = molecule
        .geometry
        .atoms
        .iter()
        .map(|atom| atom.element.atomic_number as i32)
        .sum::<i32>()
        - molecule.charge;
    if electrons <= 0 {
        return Err(MoleculeConfigError::NonPositiveElectronCount { electrons });
    }

    let spin = molecule.unpaired_electrons() as i32;
    if spin > electrons || (electrons - spin) % 2 != 0 {
        return Err(MoleculeConfigError::IncompatibleMultiplicity {
            electrons,
            multiplicity: molecule.multiplicity.get(),
        });
    }

    Ok(())
}

fn ensure_hf_converged_for_mp2(result: &ScfResult) -> miette::Result<()> {
    if result.converged {
        Ok(())
    } else {
        Err(miette::miette!(
            "MP2 requires converged HF orbitals, but HF did not converge after {} iterations",
            result.iterations
        ))
    }
}

#[derive(clap::Args, Debug)] // Allows this structure to be used with Clap
pub struct RunCommand {
    /// The toml file used for the calculation. If not specified, the standard input is used.
    pub input: Option<PathBuf>,
    /// Enable automatic download of basis sets for this execution (the default behavoir is determined by the env variable RUSTIQ_AUTO_DOWNLOAD)
    #[arg(
        long = "auto-download",
        action = ArgAction::SetTrue,
        conflicts_with = "no_auto_download"
    )]
    auto_download: bool,

    /// Disable automatic download of basis sets for this execution (the default behavoir is determined by the env variable RUSTIQ_AUTO_DOWNLOAD)
    #[arg(
        long = "no-auto-download",
        action = ArgAction::SetTrue,
        conflicts_with = "auto_download"
    )]
    no_auto_download: bool,
}

impl RunCommand {
    #[cfg(feature = "online")]
    fn resolve_auto_download(&self) -> bool {
        if self.auto_download {
            true
        } else if self.no_auto_download {
            false
        } else {
            cli::env::auto_download_value()
        }
    }

    fn resolve_basis(&self, name: &str) -> miette::Result<BasisFile> {
        let basis_store = BasisStore::default();

        cfg_if::cfg_if! {
            if #[cfg(feature = "online")] {
                if self.resolve_auto_download() {
                    self.get_basis_online(&basis_store, name)
                } else {
                    self.get_basis_offline(&basis_store, name)
                }
            } else {
                self.get_basis_offline(&basis_store, name)
            }
        }
    }

    #[cfg(feature = "online")]
    fn get_basis_online(&self, store: &BasisStore, name: &str) -> miette::Result<BasisFile> {
        store.get_or_download(name).into_diagnostic()
    }

    fn get_basis_offline(&self, store: &BasisStore, name: &str) -> miette::Result<BasisFile> {
        if let Some(basis) = store.get(name).into_diagnostic()? {
            Ok(basis)
        } else {
            Err(miette!(
                "Basis {} not found in {}",
                name,
                store.path().display()
            ))
        }
    }
}

impl Runnable for RunCommand {
    fn run(&self) -> CommandResult {
        cli::ux::print_startup_banner();
        let (source_name, toml_content) = if let Some(path_toml) = &self.input {
            let content = fs::read_to_string(path_toml).into_diagnostic()?;
            if let Some(dir) = path_toml.parent() {
                env::set_current_dir(dir).into_diagnostic()?;
            }
            (path_toml.display().to_string(), content)
        } else {
            let mut content = String::new();
            io::stdin().read_to_string(&mut content).into_diagnostic()?;
            ("<stdin>".to_string(), content)
        };
        let parsed = parse_runfile(source_name, &toml_content)?;
        let run = parsed.runfile;
        bat::print_toml(&parsed.formatted_toml);
        let molecule_path = &run.global.molecule.geometry;
        let geom = Geometry::from_path(molecule_path).into_diagnostic()?;
        let mut molecule = Molecule::try_new(
            geom,
            run.global.molecule.molecule_unit,
            run.global.molecule.charge,
            run.global.molecule.multiplicity,
        )
        .into_diagnostic()?;
        validate_molecule_config(&molecule)?;
        bat::print_xyz(&molecule.geometry.to_string());
        molecule.convert_to(Units::Bohr);
        println!("Loading basis set...");
        let step_start = Instant::now();
        let basis_file = self.resolve_basis(&run.global.basis)?;
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
            println!("Conv {}", hf.convergence_threshold.into_inner());
            println!("Max iter: {}", hf.max_iterations.get());
            println!("Preparing SCF calculation...");
            let resolved_method = hf.method.resolve(&molecule).into_diagnostic()?;
            println!("Resolved HF method: {resolved_method}");
            match resolved_method {
                ResolvedHfMethod::Rhf => {
                    let mut scf = hf::scf::ScfCalculation::new_with_progress(
                        &molecule,
                        &basis,
                        hf.max_iterations.get(),
                        hf.convergence_threshold.into_inner(),
                        hf.guess,
                        |step| println!("  {step}..."),
                    )
                    .into_diagnostic()?;
                    if hf.diis {
                        scf.enable_diis(hf.diis_size);
                    }
                    let result = match hf.format {
                        HfOutputFormat::Normal => {
                            let stdout = io::stdout();
                            let mut reporter = ScfReporter::new(stdout.lock());
                            let result = scf.run_with_observer(&mut reporter).into_diagnostic()?;
                            if let Some(err) = reporter.take_error() {
                                return Err(miette::miette!("failed to write SCF report: {err}"));
                            }
                            reporter.write_summary(&result).into_diagnostic()?;
                            result
                        }
                        HfOutputFormat::Nope => scf.run().into_diagnostic()?,
                    };

                    if let Some(mp2) = run.mp2.as_ref() {
                        ensure_hf_converged_for_mp2(&result)?;
                        let mp2_result = mp2_calc::rhf_closed_shell(&scf, mp2.frozen_orbitals)
                            .into_diagnostic()?;
                        let stdout = io::stdout();
                        let mut reporter = Mp2Reporter::new(stdout.lock(), "RHF MP2");
                        reporter
                            .write_summary(&mp2_result, &result)
                            .into_diagnostic()?;
                    }
                }
                ResolvedHfMethod::Uhf => {
                    let mut scf = hf::uhf::UhfCalculation::new_with_progress(
                        &molecule,
                        &basis,
                        hf.max_iterations.get(),
                        hf.convergence_threshold.into_inner(),
                        hf.guess,
                        |step| println!("  {step}..."),
                    )
                    .into_diagnostic()?;
                    if hf.diis {
                        scf.enable_diis(hf.diis_size.into_inner())
                            .into_diagnostic()?;
                    }
                    let result = match hf.format {
                        HfOutputFormat::Normal => {
                            let stdout = io::stdout();
                            let mut reporter = ScfReporter::new(stdout.lock());
                            let result = scf.run_with_observer(&mut reporter).into_diagnostic()?;
                            if let Some(err) = reporter.take_error() {
                                return Err(miette::miette!("failed to write SCF report: {err}"));
                            }
                            reporter.write_summary(&result).into_diagnostic()?;
                            result
                        }
                        HfOutputFormat::Nope => scf.run().into_diagnostic()?,
                    };

                    if let Some(mp2) = run.mp2.as_ref() {
                        ensure_hf_converged_for_mp2(&result)?;
                        let mp2_result = mp2_calc::uhf_unrestricted(&scf, mp2.frozen_orbitals)
                            .into_diagnostic()?;
                        let stdout = io::stdout();
                        let mut reporter = Mp2Reporter::new(stdout.lock(), "UHF MP2");
                        reporter
                            .write_summary(&mp2_result, &result)
                            .into_diagnostic()?;
                    }
                }
            }
        } else if run.mp2.is_some() {
            return Err(miette::miette!("MP2 requires an [hf] section"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::molecules::{atom::Atom, geometry::Geometry, units::Units};
    use nalgebra::point;
    #[cfg(feature = "online")]
    use rstest::rstest;
    use std::num::NonZeroU8;

    fn molecule(atom_symbols: &[&str], charge: i32, multiplicity: u8) -> Molecule {
        let elements = periodic_table::periodic_table();
        let atoms = atom_symbols
            .iter()
            .enumerate()
            .map(|(index, symbol)| {
                let element = elements
                    .iter()
                    .find(|element| element.symbol == *symbol)
                    .unwrap();
                Atom::new(element, point![0.0, 0.0, index as f64])
            })
            .collect();

        // SAFETY: These tests only build molecules whose charge does not exceed
        // their nuclear charge.
        unsafe {
            Molecule::new_unchecked(
                Geometry::new("test molecule".to_string(), atoms),
                Units::Bohr,
                charge,
                NonZeroU8::new(multiplicity).unwrap(),
            )
        }
    }

    #[test]
    fn test_validate_molecule_config_rejects_non_positive_electron_count() {
        let molecule = molecule(&["H"], 2, 1);

        assert!(matches!(
            validate_molecule_config(&molecule),
            Err(MoleculeConfigError::NonPositiveElectronCount { electrons: -1 })
        ));
    }

    #[test]
    fn test_validate_molecule_config_rejects_incompatible_multiplicity() {
        let molecule = molecule(&["H", "H"], 0, 2);

        assert!(matches!(
            validate_molecule_config(&molecule),
            Err(MoleculeConfigError::IncompatibleMultiplicity {
                electrons: 2,
                multiplicity: 2
            })
        ));
    }

    #[test]
    fn test_parse_runfile_reports_toml_span() {
        let result = parse_runfile("calculation.toml".to_string(), "hf = \"not a table\"");

        let err = result.unwrap_err();
        assert!(format!("{err:?}").contains("toml_deserialize"));
    }

    #[cfg(feature = "online")]
    #[rstest]
    #[case::unset(None, false)]
    #[case::one(Some("1"), true)]
    #[case::true_lower(Some("true"), true)]
    #[case::true_upper(Some("TRUE"), true)]
    #[case::true_mixed(Some("TrUe"), true)]
    #[case::zero(Some("0"), false)]
    #[case::false_lower(Some("false"), false)]
    #[case::false_upper(Some("FALSE"), false)]
    #[case::false_mixed(Some("FaLsE"), false)]
    #[case::garbage(Some("yes"), false)]
    fn test_resolve_auto_download_reads_env_values(
        #[case] value: Option<&str>,
        #[case] expected: bool,
    ) {
        temp_env::with_var("RUSTIQ_AUTO_DOWNLOAD", value, || {
            let command = RunCommand {
                input: None,
                auto_download: false,
                no_auto_download: false,
            };

            assert_eq!(command.resolve_auto_download(), expected);
        });
    }

    #[cfg(feature = "online")]
    #[test]
    fn test_resolve_auto_download_prefers_cli_flags_over_env() {
        temp_env::with_var("RUSTIQ_AUTO_DOWNLOAD", Some("false"), || {
            let command = RunCommand {
                input: None,
                auto_download: true,
                no_auto_download: false,
            };

            assert!(command.resolve_auto_download());
        });

        temp_env::with_var("RUSTIQ_AUTO_DOWNLOAD", Some("true"), || {
            let command = RunCommand {
                input: None,
                auto_download: false,
                no_auto_download: true,
            };

            assert!(!command.resolve_auto_download());
        });
    }

    #[cfg(feature = "online")]
    #[test]
    fn test_resolve_auto_download_defaults_to_false_when_unset_and_no_flags_are_present() {
        temp_env::with_var("RUSTIQ_AUTO_DOWNLOAD", Option::<&str>::None, || {
            let command = RunCommand {
                input: None,
                auto_download: false,
                no_auto_download: false,
            };

            assert!(!command.resolve_auto_download());
        });
    }
}
