use std::{
    fs,
    io::{self, Read, Write},
    path::PathBuf,
    time::Instant,
};

use clap::{ArgAction, ValueEnum};
use miette::{miette, Diagnostic, IntoDiagnostic, NamedSource, Report, WrapErr};

use super::batch_orchestration::{self, BatchSummary, Executable, ExecutionError, ExecutionResult};
use crate::cli::{
    self, directories,
    ux::{
        bat,
        calculation_presentation::{calculation_summary, requested_calculation},
        calculation_report::CalculationReporter,
        json_output::{BatchEntry, BatchError, BatchOutcome, BatchOutput, CalculationOutput},
    },
};
use crate::runfile::{parser::parse_runfile, resolved::ScfOutput};
use rustiq_core::{
    basis::{BasisFile, BasisStore},
    calculation::{CalculationBuilder, CalculationExecution},
    molecules::geometry::Geometry,
    persistence::EriCache,
};

use super::{CommandResult, Runnable};

#[derive(clap::Args, Debug)] // Allows this structure to be used with Clap
pub struct RunCommand {
    /// TOML or Nickel calculation input. If omitted, read TOML from standard input.
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

    /// Pretty-print JSON output and syntax-highlight it when color is enabled.
    #[arg(long)]
    pretty: bool,

    /// Directory used to cache calculation artifacts for this execution.
    #[arg(long, value_name = "DIR")]
    cache_dir: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Default, ValueEnum, PartialEq, Eq)]
pub(crate) enum CalculationOutputFormat {
    #[default]
    Text,
    Json,
}

pub(crate) fn validate_run_arguments(matches: &clap::ArgMatches) -> Result<(), clap::Error> {
    let Some(matches) = matches.subcommand_matches("run") else {
        return Ok(());
    };
    let pretty = matches.get_one::<bool>("pretty").copied().unwrap_or(false);
    let format = matches
        .get_one::<CalculationOutputFormat>("format")
        .copied()
        .unwrap_or_default();
    if pretty && format != CalculationOutputFormat::Json {
        return Err(clap::Error::raw(
            clap::error::ErrorKind::ArgumentConflict,
            "--pretty requires --format json",
        ));
    }
    Ok(())
}

#[derive(Debug, thiserror::Error, Diagnostic)]
#[error("failed to write calculation report: {0}")]
struct ReportWriteError(#[source] io::Error);

#[derive(Debug)]
enum CalculationExecutionError {
    Recoverable(Report),
    FatalInfrastructure(Report),
}

impl ExecutionError for CalculationExecutionError {
    fn is_fatal(&self) -> bool {
        matches!(self, Self::FatalInfrastructure(_))
    }
}

struct CalculationTask<'a> {
    command: &'a RunCommand,
    parsed: miette::Result<crate::runfile::parser::ParsedRunFile>,
    source_name: &'a str,
    source_content: &'a str,
    batch_position: Option<(usize, usize)>,
}

impl Executable for CalculationTask<'_> {
    type Output = CalculationOutput;
    type Error = CalculationExecutionError;

    fn execute(self) -> Result<Self::Output, Self::Error> {
        if let Some((position, total)) = self.batch_position {
            if self.command.format == CalculationOutputFormat::Text {
                writeln!(cli::color::stdout(), "\nCalculation {position}/{total}")
                    .map_err(ReportWriteError)
                    .map_err(CalculationExecutionError::fatal)?;
            }
        }
        let parsed = self
            .parsed
            .map_err(CalculationExecutionError::recoverable)?;
        let result =
            self.command
                .execute_calculation(parsed, self.source_name, self.source_content)?;
        if self.batch_position.is_some() {
            result
                .ensure_finite()
                .into_diagnostic()
                .map_err(CalculationExecutionError::recoverable)?;
        }
        Ok(result)
    }
}

impl CalculationExecutionError {
    fn recoverable(error: impl Into<Report>) -> Self {
        Self::Recoverable(error.into())
    }
    fn fatal(error: impl Into<Report>) -> Self {
        Self::FatalInfrastructure(error.into())
    }
    fn into_report(self) -> Report {
        match self {
            Self::Recoverable(error) | Self::FatalInfrastructure(error) => error,
        }
    }
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

    #[cfg_attr(
        not(feature = "online"),
        allow(
            clippy::unused_self,
            reason = "Online builds use the command download flags; offline builds share the same resolver"
        )
    )]
    fn resolve_basis(&self, name: &str) -> miette::Result<BasisFile> {
        let basis_store = directories::basis_store();

        cfg_if::cfg_if! {
            if #[cfg(feature = "online")] {
                if self.resolve_auto_download() {
                    Self::get_basis_online(&basis_store, name)
                } else {
                    Self::get_basis_offline(&basis_store, name)
                }
            } else {
                Self::get_basis_offline(&basis_store, name)
            }
        }
    }

    #[cfg(feature = "online")]
    fn get_basis_online(store: &BasisStore, name: &str) -> miette::Result<BasisFile> {
        store.get_or_download(name).into_diagnostic()
    }

    fn get_basis_offline(store: &BasisStore, name: &str) -> miette::Result<BasisFile> {
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
            cli::ux::print_startup_banner().into_diagnostic()?;
        }
        let (source_name, source_content) = if let Some(path_toml) = &self.input {
            let content = fs::read_to_string(path_toml).into_diagnostic()?;
            (path_toml.display().to_string(), content)
        } else {
            let mut content = String::new();
            io::stdin().read_to_string(&mut content).into_diagnostic()?;
            ("<stdin>".to_string(), content)
        };
        let nickel = self
            .input
            .as_ref()
            .and_then(|path| path.extension())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("ncl"));
        if !nickel {
            let parsed = parse_runfile(source_name.clone(), &source_content)?;
            return self.execute_single(parsed, &source_name, &source_content, json_output);
        }
        let resolved = crate::runfile::parser::resolve_nickel(&source_name, &source_content)?;
        if resolved.calculations().len() == 1 {
            let calculation = resolved.into_iter().next().ok_or_else(|| {
                miette!("validated Nickel input unexpectedly contained no calculations")
            })?;
            let parsed = crate::runfile::parser::parsed_calculation(calculation)?;
            return self.execute_single(parsed, &source_name, &source_content, json_output);
        }
        self.execute_batch(resolved, &source_name, &source_content, json_output)
    }
}

impl RunCommand {
    fn execute_single(
        &self,
        parsed: crate::runfile::parser::ParsedRunFile,
        source_name: &str,
        source_content: &str,
        json_output: bool,
    ) -> CommandResult {
        let result = CalculationTask {
            command: self,
            parsed: Ok(parsed),
            source_name,
            source_content,
            batch_position: None,
        }
        .execute()
        .map_err(CalculationExecutionError::into_report)?;
        if json_output {
            let mut json = Vec::new();
            result
                .write_json(&mut json, self.pretty)
                .into_diagnostic()?;
            if self.pretty {
                bat::print_json(&json)?;
            } else {
                let mut stdout = io::stdout().lock();
                stdout.write_all(&json).into_diagnostic()?;
                writeln!(stdout).into_diagnostic()?;
            }
        }
        Ok(())
    }

    fn execute_batch(
        &self,
        resolved: crate::runfile::resolved::ResolvedInput,
        source_name: &str,
        source_content: &str,
        json_output: bool,
    ) -> CommandResult {
        let total = resolved.calculations().len();
        let tasks = resolved
            .into_iter()
            .enumerate()
            .map(|(index, calculation)| CalculationTask {
                command: self,
                parsed: crate::runfile::parser::parsed_calculation(calculation),
                source_name,
                source_content,
                batch_position: Some((index + 1, total)),
            });
        let outcomes = batch_orchestration::execute_batch(tasks)
            .map_err(CalculationExecutionError::into_report)?;

        let BatchSummary {
            succeeded,
            non_converged,
            failed,
        } = BatchSummary::from_results(&outcomes);
        let entries = outcomes
            .into_iter()
            .enumerate()
            .map(|(index, outcome)| {
                let outcome = match outcome {
                    Ok(result) if result.is_converged() => BatchOutcome::Success { result },
                    Ok(result) => BatchOutcome::NonConverged { result },
                    Err(error) => {
                        let error = error.into_report();
                        if !json_output {
                            writeln!(
                                io::stderr().lock(),
                                "Calculation {}: {:?}",
                                index + 1,
                                error
                            )
                            .into_diagnostic()?;
                        }
                        BatchOutcome::Error {
                            error: BatchError {
                                message: format!("{error}"),
                            },
                        }
                    }
                };
                Ok(BatchEntry { outcome })
            })
            .collect::<miette::Result<Vec<_>>>()?;
        if json_output {
            let batch = BatchOutput {
                schema_version: 1,
                kind: "batch",
                calculations: entries,
            };
            let json = if self.pretty {
                serde_json::to_vec_pretty(&batch)
            } else {
                serde_json::to_vec(&batch)
            }
            .into_diagnostic()?;
            if self.pretty {
                bat::print_json(&json)?;
            } else {
                let mut stdout = io::stdout().lock();
                stdout.write_all(&json).into_diagnostic()?;
                writeln!(stdout).into_diagnostic()?;
            }
        } else {
            writeln!(
                cli::color::stdout(),
                "\nBatch: {succeeded} succeeded, {non_converged} non-converged, {failed} failed"
            )
            .into_diagnostic()?;
        }
        if failed + non_converged > 0 {
            return Err(miette!(
                "batch contains {failed} failed and {non_converged} non-converged calculations"
            ));
        }
        Ok(())
    }
}

impl RunCommand {
    #[allow(
        clippy::too_many_lines,
        reason = "Keep preparation and execution in their scientific order"
    )]
    fn execute_calculation(
        &self,
        parsed: crate::runfile::parser::ParsedRunFile,
        source_name: &str,
        source_content: &str,
    ) -> Result<CalculationOutput, CalculationExecutionError> {
        let json_output = self.format == CalculationOutputFormat::Json;
        let run = parsed
            .resolved
            .single_calculation()
            .into_diagnostic()
            .map_err(CalculationExecutionError::recoverable)?;
        let molecule_path = run.resource_path(self.input.as_deref());
        let xyz_content = fs::read_to_string(&molecule_path)
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to read geometry {}", molecule_path.display()))
            .map_err(CalculationExecutionError::recoverable)?;
        let geom = Geometry::from_source(molecule_path.display().to_string(), &xyz_content)
            .into_diagnostic()
            .map_err(CalculationExecutionError::recoverable)?;
        if !json_output {
            let mut stdout = cli::color::stdout();
            writeln!(stdout, "{}", cli::color::title("Loading basis set..."))
                .map_err(ReportWriteError)
                .map_err(CalculationExecutionError::fatal)?;
        }
        let step_start = Instant::now();
        let basis_file = self
            .resolve_basis(&run.basis.name)
            .map_err(CalculationExecutionError::recoverable)?;
        if !json_output {
            let mut stdout = cli::color::stdout();
            writeln!(
                stdout,
                "{} {:?}",
                cli::color::value(basis_file.name()),
                basis_file.function_types()
            )
            .map_err(ReportWriteError)
            .map_err(CalculationExecutionError::fatal)?;
            writeln!(
                stdout,
                "{} {}",
                cli::color::title("Basis file loaded in"),
                humantime::format_duration(step_start.elapsed())
            )
            .map_err(ReportWriteError)
            .map_err(CalculationExecutionError::fatal)?;
        }
        let show_scf = run.output.scf != ScfOutput::Quiet;
        let calculation = CalculationBuilder::new(&geom, &basis_file)
            .with_basis_label(&run.basis.name)
            .with_molecule_config(parsed.molecule_config)
            .with_integrals(parsed.integral_config)
            .with_mp2(parsed.mp2_config);
        let calculation = if run.cache.enabled {
            let cache_root = self
                .cache_dir
                .clone()
                .unwrap_or_else(cli::directories::cache_path);
            calculation.with_eri_cache(EriCache::new(cache_root))
        } else {
            calculation
        };
        let calculation = if let Some(hf_config) = parsed.hf_config {
            calculation.with_hf(hf_config)
        } else {
            calculation
        };
        let prepared = {
            let stdout = cli::color::stdout();
            let mut reporter = CalculationReporter::new(stdout.lock(), !json_output, show_scf);
            let prepared = calculation.prepare_with_events(|event| reporter.on_event(event));
            if let Some(error) = reporter.take_error() {
                return Err(CalculationExecutionError::fatal(ReportWriteError(error)));
            }
            prepared
                .map_err(|error| with_input_source(error, source_name, source_content))
                .map_err(CalculationExecutionError::recoverable)?
        };
        if !json_output {
            let output_format = match run.output.scf {
                ScfOutput::Normal => crate::runfile::output::ScfOutput::Normal,
                ScfOutput::Quiet => crate::runfile::output::ScfOutput::Quiet,
            };
            let requested =
                requested_calculation(prepared.request(), run.cache.enabled, output_format)
                    .into_diagnostic()
                    .map_err(CalculationExecutionError::recoverable)?;
            {
                let mut stdout = cli::color::stdout();
                writeln!(
                    stdout,
                    "\n{}",
                    cli::color::title("Requested calculation (canonical TOML)")
                )
                .map_err(ReportWriteError)
                .map_err(CalculationExecutionError::fatal)?;
            }
            bat::print_toml(&requested.toml);
            {
                let mut stdout = cli::color::stdout();
                writeln!(
                    stdout,
                    "\n{}",
                    cli::color::title(format!(
                        "Requested geometry (canonical XYZ, {})",
                        requested.units
                    ))
                )
                .map_err(ReportWriteError)
                .map_err(CalculationExecutionError::fatal)?;
            }
            bat::print_xyz(&requested.xyz);
            writeln!(
                cli::color::stdout(),
                "\n{}",
                calculation_summary(&prepared, &molecule_path)
            )
            .map_err(ReportWriteError)
            .map_err(CalculationExecutionError::fatal)?;
        }
        let result = {
            let stdout = cli::color::stdout();
            let mut reporter = CalculationReporter::new(stdout.lock(), !json_output, show_scf);
            let outcome = prepared.execute_with_events(|event| reporter.on_event(event));
            if let Some(error) = reporter.take_error() {
                return Err(CalculationExecutionError::fatal(ReportWriteError(error)));
            }
            outcome
                .map_err(|error| with_input_source(error, source_name, source_content))
                .map_err(CalculationExecutionError::recoverable)?
        };
        Ok(CalculationOutput::from(&result))
    }
}

fn with_source<E>(error: E, source: &NamedSource<String>) -> Report
where
    E: Diagnostic + Send + Sync + 'static,
{
    Report::new(error).with_source_code(source.clone())
}

fn with_input_source<E>(error: E, name: &str, content: &str) -> Report
where
    E: Diagnostic + Send + Sync + 'static,
{
    let source = NamedSource::new(name, content.to_owned());
    with_source(error, &source)
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
        assert!(format!("{err:?}").contains("nickel"));
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
                pretty: false,
                cache_dir: None,
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
                pretty: false,
                cache_dir: None,
            };

            assert!(command.resolve_auto_download());
        });

        temp_env::with_var("RUSTIQ_AUTO_DOWNLOAD", Some("true"), || {
            let command = RunCommand {
                input: None,
                auto_download: false,
                no_auto_download: true,
                format: CalculationOutputFormat::Text,
                pretty: false,
                cache_dir: None,
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
                pretty: false,
                cache_dir: None,
            };

            assert!(!command.resolve_auto_download());
        });
    }
}
