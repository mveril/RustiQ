//! Execute a campaign against read-only portable calculation entries.
mod method;
mod report;

use crate::cli::ux::diagnostics::with_code;
use std::{fs, path::Path};

use miette::{miette, Diagnostic, IntoDiagnostic, Report, WrapErr};
use rustiq_core::persistence::{
    Artifact, ArtifactError, ConvergedHfArtifact, PortableError, PostScfSession, RustiQBundle,
    RustiQData,
};

use self::method::{Method, Request, ScientificResult};
use super::run_command::CalculationOutputFormat;
use crate::cli::exit_status::IncompleteBatch;

#[derive(Default, serde::Serialize)]
struct Summary {
    succeeded: usize,
    skipped: usize,
    failed: usize,
}

struct Campaign {
    summary: Summary,
    calculations: Vec<Calculation>,
}

struct Calculation {
    source_index: usize,
    methods: Vec<(Method, Outcome)>,
}

enum Outcome {
    Succeeded(ScientificResult),
    Skipped(Skip),
    Failed(Report),
}

#[derive(Clone, Debug)]
struct Skip {
    reason: String,
    help: &'static str,
}

#[derive(Debug, thiserror::Error, Diagnostic)]
#[error("No requested Post-SCF calculation succeeded. {succeeded} succeeded, {skipped} skipped, {failed} failed.")]
#[diagnostic(code(rustiq::post_scf::no_successful_calculations))]
struct NoSuccess {
    succeeded: usize,
    skipped: usize,
    failed: usize,
}

#[derive(Debug)]
enum RestoreError {
    Skipped(Skip),
    Failed,
}

impl From<&PortableError> for RestoreError {
    fn from(error: &PortableError) -> Self {
        let help = match error {
            PortableError::MissingArtifact(_) => {
                "Create a bundle with converged HF and AO ERIs using rustiq run --artifact."
            }
            PortableError::IncompatibleEri
            | PortableError::Artifact(ArtifactError::UnsupportedRepresentation(_)) => {
                "Create a bundle with a supported reference and matching artifacts."
            }
            _ => return Self::Failed,
        };
        Self::Skipped(Skip {
            reason: match error {
                PortableError::MissingArtifact(ConvergedHfArtifact::NAME) => {
                    "no persisted converged HF solution is available".to_owned()
                }
                _ => error.to_string(),
            },
            help,
        })
    }
}

pub(super) fn run(
    input: Option<&Path>,
    config: &Path,
    source_index: Option<usize>,
    methods: &[String],
    (artifact, force): (Option<&Path>, bool),
    (format, pretty): (CalculationOutputFormat, bool),
) -> miette::Result<()> {
    let input = input
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("rustiq"))
        })
        .ok_or_else(|| {
            miette!(
                code = "rustiq::post_scf::input_required",
                "--post-scf requires a .rustiq input bundle"
            )
        })?;
    let destination = artifact
        .map(|path| {
            let (path, replace) =
                super::artifact_session::resolve_artifact_destination(None, path, force)?;
            if replace && same_file::is_same_file(input, &path).into_diagnostic()? {
                return Err(miette!(
                    code = "rustiq::post_scf::output_is_input",
                    "Post-HF output must not replace the input bundle"
                ));
            }
            Ok((path, replace))
        })
        .transpose()?;
    let source = fs::read_to_string(config)
        .into_diagnostic()
        .wrap_err_with(|| format!("could not read Post-HF configuration {}", config.display()))
        .map_err(|report| with_code(report, "rustiq::post_scf::config_read_failed"))?;
    let settings = crate::runfile::parser::resolve_post_scf(&config.to_string_lossy(), &source)?;
    let requests = Request::select(&settings, methods)?;
    let bundle = RustiQBundle::open_for_restore(input)
        .into_diagnostic()
        .map_err(|report| with_code(report, "rustiq::artifact::open_failed"))
        .wrap_err_with(|| format!("could not open Post-HF input bundle {}", input.display()))?;
    let calculations = bundle.calculations();
    if let Some(index) = source_index {
        if index >= calculations.len() {
            return Err(miette!(
                code = "rustiq::post_scf::calculation_out_of_range",
                "Calculation index {index} is out of range. Use a zero-based index from 0 to {} or omit --calculation.",
                calculations.len() - 1
            ));
        }
    }
    let campaign = execute(
        calculations
            .iter()
            .enumerate()
            .filter(|(index, _)| source_index.is_none_or(|selected| *index == selected)),
        &requests,
    );
    campaign.print(format, pretty)?;
    let status = campaign.finish();
    let publication = destination.as_ref().map_or(Ok(()), |(path, replace)| {
        publish_artifact(path, &bundle, &campaign, (config, &source), *replace)
    });
    campaign.emit_diagnostics((config, &source))?;
    publication?;
    status
}

fn publish_artifact(
    path: &Path,
    source: &RustiQBundle,
    campaign: &Campaign,
    config: (&Path, &str),
    replace: bool,
) -> miette::Result<()> {
    let entries = campaign
        .calculations
        .iter()
        .filter_map(|calculation| {
            let mut results = calculation
                .methods
                .iter()
                .filter_map(|(_, outcome)| match outcome {
                    Outcome::Succeeded(result) => Some(result),
                    _ => None,
                });
            let first = results.next()?;
            let data = source.calculations()[calculation.source_index].clone();
            // Each supported method owns a single settings table. Methods that share
            // settings must agree before their snapshots can be combined.
            Some(first.update_snapshot(data).and_then(|data| {
                results.try_fold(data, |data, result| result.update_snapshot(data))
            }))
        })
        .collect::<Result<Vec<_>, _>>()
        .into_diagnostic()?;
    if entries.is_empty() {
        return Ok(());
    }
    let mut output = source.with_calculations(entries).into_diagnostic()?;
    output
        .add_source(config.0.to_string_lossy(), config.1.as_bytes())
        .into_diagnostic()?;
    (if replace {
        output.replace(path)
    } else {
        output.write(path)
    })
    .into_diagnostic()
    .wrap_err_with(|| format!("could not publish Post-HF artifact {}", path.display()))
}

fn execute<'a>(
    entries: impl Iterator<Item = (usize, &'a RustiQData)>,
    requests: &[Request],
) -> Campaign {
    let mut summary = Summary::default();
    let calculations = entries
        .map(|(source_index, data)| {
            let mut session = PostScfSession::new(data);
            let methods = requests
                .iter()
                .map(|request| {
                    let outcome = request.execute(&mut session);
                    match &outcome {
                        Outcome::Succeeded(_) => summary.succeeded += 1,
                        Outcome::Skipped(_) => summary.skipped += 1,
                        Outcome::Failed(_) => summary.failed += 1,
                    }
                    (request.method(), outcome)
                })
                .collect();
            Calculation {
                source_index,
                methods,
            }
        })
        .collect();
    Campaign {
        summary,
        calculations,
    }
}

impl Campaign {
    fn finish(&self) -> miette::Result<()> {
        let Summary {
            succeeded,
            skipped,
            failed,
        } = self.summary;
        if succeeded == 0 {
            return Err(NoSuccess {
                succeeded,
                skipped,
                failed,
            }
            .into());
        }
        if failed != 0 {
            return Err(IncompleteBatch {
                succeeded,
                non_converged: 0,
                failed,
                general_errors: failed,
            }
            .into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustiq_core::{
        config::{HfConfig, HfMethod, Mp2Config},
        persistence::{ConvergedHf, ConvergedHfArtifact},
        prelude::*,
    };

    fn entry(method: HfMethod, save_hf: bool) -> RustiQData {
        let geometry = Geometry::from_source("h2.xyz", "2\nH2\nH 0 0 0\nH 1.4 0 0\n").unwrap();
        let basis =
            BasisFile::from_reader(&include_bytes!("../../../tests/data/sto-3g.json")[..]).unwrap();
        let mut hf = HfConfig::default();
        hf.method = method.into();
        let prepared = CalculationBuilder::new(&geometry, &basis)
            .with_hf(hf)
            .prepare()
            .unwrap();
        let mut data = RustiQData::from_calculation(&prepared).unwrap();
        if save_hf {
            let solution = prepared.run_hf().unwrap().into_converged().unwrap();
            data.set::<ConvergedHfArtifact>(&ConvergedHf::from_solution(&solution).unwrap())
                .unwrap();
            data.set_eri(solution.ao_eri()).unwrap();
        }
        data
    }

    #[test]
    fn methods_keep_independent_outcomes() {
        let entries = [
            entry(HfMethod::Rhf, true),
            entry(HfMethod::Uhf, true),
            entry(HfMethod::Rhf, false),
        ];
        // Exercise the same lazy, read-only archive access used by the CLI.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("campaign.rustiq");
        RustiQBundle::new(Vec::from(entries))
            .unwrap()
            .write(&path)
            .unwrap();
        let bundle = RustiQBundle::open_for_restore(path).unwrap();
        let requests = [Request::RestrictedProbe, Request::Mp2(Mp2Config::default())];
        let campaign = execute(bundle.calculations().iter().enumerate(), &requests);
        assert_eq!(campaign.summary.succeeded, 3);
        assert_eq!(campaign.summary.skipped, 3);
        assert_eq!(campaign.summary.failed, 0);
        assert!(campaign.finish().is_ok());
        let [rhf, uhf, missing]: &[_; 3] = campaign.calculations.as_slice().try_into().unwrap();
        assert!(rhf
            .methods
            .iter()
            .all(|(_, result)| matches!(result, Outcome::Succeeded(_))));
        assert!(matches!(
            uhf.methods[0],
            (Method::RestrictedProbe, Outcome::Skipped(_))
        ));
        assert!(matches!(
            uhf.methods[1],
            (Method::Mp2, Outcome::Succeeded(_))
        ));
        for (method, outcome) in &missing.methods {
            assert!(
                matches!(outcome, Outcome::Skipped(skip) if skip.reason.contains("no persisted converged HF"))
            );
            if let Outcome::Skipped(skip) = outcome {
                let rendered = report::skip_message(missing.source_index, *method, skip.clone());
                assert!(rendered.contains(&format!("Calculation 2, method {}", method.name())));
            }
        }

        let selected = execute([(1, &bundle.calculations()[1])].into_iter(), &requests[..1]);
        assert_eq!(selected.summary.skipped, 1);
        assert!(selected.finish().unwrap_err().is::<NoSuccess>());

        // A method execution error does not prevent a later method on the same reference.
        let mut invalid = Mp2Config::default();
        invalid.frozen_orbitals = 1.into();
        let requests = [Request::Mp2(invalid), Request::RestrictedProbe];
        let selected = execute([(0, &bundle.calculations()[0])].into_iter(), &requests);
        assert_eq!(selected.summary.failed, 1);
        assert_eq!(selected.summary.succeeded, 1);
        let status = selected.finish().unwrap_err();
        assert!(status.is::<IncompleteBatch>());
        assert_eq!(
            crate::cli::exit_status::for_error(&status),
            std::process::ExitCode::from(4)
        );
    }

    #[test]
    fn an_empty_plan_cannot_succeed() {
        let campaign = execute(std::iter::empty(), &[]);
        assert!(campaign.finish().unwrap_err().is::<NoSuccess>());
    }
}
