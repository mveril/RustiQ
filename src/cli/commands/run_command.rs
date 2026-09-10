use std::{
    env, fs,
    io::{self, Read},
    path::PathBuf,
    time::Instant,
};

use clap::{ArgAction, ValueEnum};
use miette::{miette, IntoDiagnostic, NamedSource};

use crate::cli::{
    self,
    ux::{bat, calculation_report::CalculationReporter, json_output::CalculationOutput},
};
use crate::runfile::{hf::HfOutputFormat, parser::parse_runfile};
use rustiq_core::{
    basis::{BasisFile, BasisStore},
    calculation::{CalculationBuilder, CalculationExecution},
    molecules::geometry::Geometry,
};

use super::{CommandResult, Runnable};

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

    /// Select the calculation-result output format. JSON writes only the versioned machine-readable result to stdout.
    #[arg(long, value_enum, default_value_t = CalculationOutputFormat::Text)]
    format: CalculationOutputFormat,
}

#[derive(Clone, Copy, Debug, Default, ValueEnum, PartialEq, Eq)]
enum CalculationOutputFormat {
    #[default]
    Text,
    Json,
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
        let basis_store = crate::cli::env::basis_store();

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
        let json_output = self.format == CalculationOutputFormat::Json;
        if !json_output {
            cli::ux::print_startup_banner();
        }
        let (source_name, toml_content) = if let Some(path_toml) = &self.input {
            let content = fs::read_to_string(path_toml).into_diagnostic()?;
            if let Some(dir) = path_toml.parent().filter(|dir| !dir.as_os_str().is_empty()) {
                env::set_current_dir(dir).into_diagnostic()?;
            }
            (path_toml.display().to_string(), content)
        } else {
            let mut content = String::new();
            io::stdin().read_to_string(&mut content).into_diagnostic()?;
            ("<stdin>".to_string(), content)
        };
        let parsed = parse_runfile(source_name.clone(), &toml_content)?;
        let source_code = NamedSource::new(source_name, toml_content);
        let scientific_error =
            |error| miette::Report::new(error).with_source_code(source_code.clone());
        let run = parsed.runfile;
        if !json_output {
            bat::print_toml(&parsed.formatted_toml);
        }
        let molecule_path = &run.global.molecule.geometry;
        let geom = Geometry::from_path(molecule_path).into_diagnostic()?;
        if !json_output {
            bat::print_xyz(&geom.to_string());
        }
        if !json_output {
            println!("Loading basis set...");
        }
        let step_start = Instant::now();
        let basis_file = self.resolve_basis(&run.global.basis)?;
        if !json_output {
            println!("{} {:?}", basis_file.name(), basis_file.function_types());
            println!(
                "Basis file loaded in {}",
                humantime::format_duration(step_start.elapsed())
            );
        }
        let show_scf = run
            .hf
            .as_ref()
            .is_none_or(|hf| hf.format != HfOutputFormat::Nope);
        let calculation = CalculationBuilder::new(&geom, &basis_file)
            .with_molecule_config(parsed.molecule_config)
            .with_mp2(parsed.mp2_config);
        let calculation = if let Some(hf_config) = parsed.hf_config {
            calculation.with_hf(hf_config)
        } else {
            calculation
        };
        let result = {
            let stdout = io::stdout();
            let mut reporter = CalculationReporter::new(stdout.lock(), !json_output, show_scf);
            let outcome = calculation.execute_with_events(|event| reporter.on_event(event));
            if let Some(error) = reporter.take_error() {
                return Err(miette!("failed to write calculation report: {error}"));
            }
            outcome.map_err(&scientific_error)?
        };
        if json_output {
            let stdout = io::stdout();
            CalculationOutput::new(
                result.hf.summary().method,
                &result.hf.summary().scf,
                result.mp2.as_ref(),
            )
            .write_json(stdout.lock())
            .into_diagnostic()?;
            println!();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "online")]
    use rstest::rstest;

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
                format: CalculationOutputFormat::Text,
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
                format: CalculationOutputFormat::Text,
            };

            assert!(command.resolve_auto_download());
        });

        temp_env::with_var("RUSTIQ_AUTO_DOWNLOAD", Some("true"), || {
            let command = RunCommand {
                input: None,
                auto_download: false,
                no_auto_download: true,
                format: CalculationOutputFormat::Text,
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
                format: CalculationOutputFormat::Text,
            };

            assert!(!command.resolve_auto_download());
        });
    }
}
