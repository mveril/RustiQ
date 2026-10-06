use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
#[error("{message}")]
#[diagnostic(code(rustiq::runfile::nickel))]
pub(crate) struct NickelRunfileDiagnostic {
    #[source_code]
    source_code: NamedSource<String>,
    #[label("{label}")]
    span: Option<SourceSpan>,
    message: String,
    label: String,
}

pub(crate) fn nickel_error(
    source_name: &str,
    source: &str,
    message: String,
    label: String,
    span: Option<SourceSpan>,
) -> NickelRunfileDiagnostic {
    NickelRunfileDiagnostic {
        source_code: NamedSource::new(source_name, source.to_owned()),
        span,
        message: if span.is_some() {
            message
        } else {
            format!("{source_name}: {message}\n{label}")
        },
        label,
    }
}

#[derive(Debug, Error, Diagnostic)]
#[error("runfile contains {count} configuration error(s)")]
#[diagnostic(
    code(rustiq::runfile::nickel),
    help("Fix each reported runfile field error.")
)]
struct NickelRunfileErrors {
    count: usize,
    #[related]
    diagnostics: Vec<NickelRunfileDiagnostic>,
}

pub(crate) fn group_nickel_errors(mut diagnostics: Vec<NickelRunfileDiagnostic>) -> miette::Report {
    if diagnostics.len() == 1 {
        diagnostics.remove(0).into()
    } else {
        NickelRunfileErrors {
            count: diagnostics.len(),
            diagnostics,
        }
        .into()
    }
}

#[cfg(test)]
#[derive(Debug, Error, Diagnostic)]
#[error("runfile contains {count} configuration error(s)")]
#[diagnostic(
    code(rustiq::runfile::toml_deserialize),
    help("Fix each reported runfile field error.")
)]
struct RunfileDeserializationError {
    count: usize,
    #[related]
    diagnostics: Vec<RunfileFieldDiagnostic>,
}

#[cfg(test)]
#[derive(Debug, Error, Diagnostic)]
#[error("{message}")]
#[diagnostic(code(rustiq::runfile::invalid_field))]
struct RunfileFieldDiagnostic {
    message: String,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("{label}")]
    span: SourceSpan,
    label: String,
}

#[cfg(test)]
pub(crate) trait FromTomlErrorMietteExt {
    fn into_miette_diagnostic(self, source_name: String, toml_content: &str) -> miette::Report;
}

#[cfg(test)]
impl FromTomlErrorMietteExt for toml_spanner::FromTomlError {
    fn into_miette_diagnostic(self, source_name: String, toml_content: &str) -> miette::Report {
        let source_code = NamedSource::new(source_name, toml_content.to_string());
        let diagnostics: Vec<RunfileFieldDiagnostic> = self
            .errors
            .iter()
            .map(|error| {
                let (span, default_label) = error
                    .primary_label()
                    .unwrap_or_else(|| (error.span(), error.message(toml_content)));
                let path = error
                    .path()
                    .map(std::string::ToString::to_string)
                    .or_else(|| path_for_span(toml_content, span));
                let (message, label) = humanized_runfile_error(
                    path.as_deref(),
                    &error.message(toml_content),
                    &default_label,
                );

                RunfileFieldDiagnostic {
                    message,
                    source_code: source_code.clone(),
                    span: source_span(span),
                    label,
                }
            })
            .collect();

        RunfileDeserializationError {
            count: diagnostics.len(),
            diagnostics,
        }
        .into()
    }
}

#[cfg(test)]
fn source_span(span: toml_spanner::Span) -> SourceSpan {
    let start = span.start as usize;
    let end = span.end as usize;
    (start, end.saturating_sub(start)).into()
}

#[cfg(test)]
fn path_for_span(toml_content: &str, span: toml_spanner::Span) -> Option<String> {
    let offset = (span.start as usize).min(toml_content.len());
    let before = toml_content.get(..offset)?;
    let after = toml_content.get(offset..)?;
    let line_start = before.rfind('\n').map_or(0, |index| index + 1);
    let line_end = after
        .find('\n')
        .map_or(toml_content.len(), |index| offset + index);
    let line = toml_content.get(line_start..line_end)?.trim();
    let key = line.split_once('=')?.0.trim();
    if key.is_empty() {
        return None;
    }

    let section = toml_content
        .get(..line_start)?
        .lines()
        .rev()
        .find_map(|line| {
            let line = line.trim();
            line.strip_prefix('[')
                .and_then(|line| line.strip_suffix(']'))
                .map(str::trim)
                .filter(|section| !section.is_empty())
        });

    Some(match section {
        Some(section) => format!("{section}.{key}"),
        None => key.to_string(),
    })
}

#[allow(
    clippy::case_sensitive_file_extension_comparisons,
    reason = "These are case-sensitive TOML field paths, not filesystem extensions"
)]
pub(crate) fn humanized_runfile_error(
    path: Option<&str>,
    raw_message: &str,
    default_label: &str,
) -> (String, String) {
    let Some(path) = path else {
        return (
            "The runfile is not valid TOML.".to_string(),
            default_label.trim().to_string(),
        );
    };

    match path {
        "basis.name" => (
            "The basis set must be written as a string.".to_string(),
            "expected a basis set name, for example name = \"sto-3g\"".to_string(),
        ),
        "molecule.geometry" => (
            "The molecule geometry path must be a non-empty string.".to_string(),
            "expected a geometry file path".to_string(),
        ),
        "molecule.charge" => (
            "The molecule charge must be an integer.".to_string(),
            "expected an integer charge".to_string(),
        ),
        "molecule.multiplicity" => (
            "The molecule multiplicity must be an integer greater than zero.".to_string(),
            "expected a positive spin multiplicity".to_string(),
        ),
        "molecule.units" => (
            "The molecule unit must be one of the supported unit names.".to_string(),
            "expected Bohr or Angstrom".to_string(),
        ),
        "method.hf.max_iterations" => (
            "The HF iteration limit must be an integer greater than zero.".to_string(),
            "expected a positive iteration count".to_string(),
        ),
        "method.hf.convergence_threshold" => (
            "The HF convergence threshold must be a positive finite number.".to_string(),
            "expected a positive finite threshold".to_string(),
        ),
        "method.hf.orthogonalization.linear_dependency_threshold" => (
            "The HF linear dependency threshold must be a non-negative finite number.".to_string(),
            "expected a non-negative finite threshold".to_string(),
        ),
        "integrals.schwarz_threshold" => (
            "The ERI Schwarz threshold must be a non-negative finite number.".to_string(),
            "expected a non-negative finite threshold".to_string(),
        ),
        "method.hf.diis.enabled" => (
            "The DIIS flag must be a boolean.".to_string(),
            "expected true or false".to_string(),
        ),
        "method.hf.diis.max_history" => (
            "The DIIS history size must be an integer greater than or equal to 2.".to_string(),
            "expected a DIIS history size of at least 2".to_string(),
        ),
        "output.scf" => (
            "The HF output format must be one of the supported format names.".to_string(),
            "expected Normal or Quiet".to_string(),
        ),
        "method.mp2.frozen_orbitals" => (
            "The MP2 frozen orbital count must be a non-negative integer.".to_string(),
            "expected a count of frozen orbitals".to_string(),
        ),
        "method.mp2.memory_limit" => (
            "The MP2 memory limit must be a positive byte size or auto.".to_string(),
            "expected a byte size such as \"512 MiB\" or \"auto\"".to_string(),
        ),
        "method.hf.guess" => (
            "The HF density guess must be configured as a table.".to_string(),
            "expected a density guess configuration".to_string(),
        ),
        _ if path.ends_with(".std_dev") => (
            "The normal distribution standard deviation must be positive and finite.".to_string(),
            "expected a positive finite standard deviation".to_string(),
        ),
        _ if path.ends_with(".min") => (
            "The uniform distribution minimum must be a finite number.".to_string(),
            "expected a finite lower bound".to_string(),
        ),
        _ if path.ends_with(".max") => (
            "The uniform distribution maximum must be finite and greater than the minimum."
                .to_string(),
            "expected a valid upper bound".to_string(),
        ),
        _ => (
            format!("The value at `{path}` is invalid."),
            raw_message.trim().to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use crate::runfile::parser::parse_runfile;

    #[test]
    fn path_for_span_handles_utf8_boundaries() {
        let source = "# é\n[global]\nbasis = 4\n";
        let start = u32::try_from(source.find('4').unwrap()).unwrap();
        assert_eq!(
            super::path_for_span(
                source,
                toml_spanner::Span {
                    start,
                    end: start + 1
                }
            ),
            Some("global.basis".to_string())
        );
        assert_eq!(
            super::path_for_span(source, toml_spanner::Span { start: 3, end: 4 }),
            None
        );
    }

    #[test]
    fn nickel_diagnostic_reports_the_original_source_location() {
        let err = parse_runfile("calculation.toml", "[basis]\nname = 4\n").unwrap_err();
        assert_eq!(err.code().unwrap().to_string(), "rustiq::runfile::nickel");
        let labels = err.labels().unwrap().collect::<Vec<_>>();
        assert_eq!(labels.len(), 1);
        let contents = err
            .source_code()
            .unwrap()
            .read_span(labels[0].inner(), 0, 0)
            .unwrap();
        assert_eq!(std::str::from_utf8(contents.data()).unwrap(), "4");
        assert_eq!(contents.name(), Some("calculation.toml"));
    }

    #[test]
    fn grouped_contract_errors_retain_each_original_value_span() {
        let source = include_str!("../../samples/invalid_diagnostics.toml");
        let error = parse_runfile("calculation.toml", source).unwrap_err();
        assert_eq!(
            error.to_string(),
            "runfile contains 4 configuration error(s)"
        );
        let mut values = error
            .related()
            .unwrap()
            .map(|diagnostic| {
                let label = diagnostic.labels().unwrap().next().unwrap();
                let contents = diagnostic
                    .source_code()
                    .unwrap()
                    .read_span(label.inner(), 0, 0)
                    .unwrap();
                assert_eq!(contents.name(), Some("calculation.toml"));
                std::str::from_utf8(contents.data()).unwrap().to_owned()
            })
            .collect::<Vec<_>>();
        values.sort();
        assert_eq!(values, ["\"one\"", "0", "0.0", "4"]);
    }

    #[test]
    fn grouping_includes_parent_contracts_missing_fields_and_domain_errors() {
        let source = "[basis]\n[method.hf]\nmax_iterations = 0\n[method.hf.guess]\ntype = 'Unknown'\n[method.mp2]\nmemory_limit = 'nonsense'\n";
        let error = parse_runfile("calculation.toml", source).unwrap_err();
        assert_eq!(
            error.to_string(),
            "runfile contains 4 configuration error(s)"
        );
        let messages = error
            .related()
            .unwrap()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        assert!(messages.iter().any(|message| message.contains("basis set")));
        assert!(messages
            .iter()
            .any(|message| message.contains("iteration limit")));
        assert!(messages
            .iter()
            .any(|message| message.contains("density guess")));
        assert!(messages
            .iter()
            .any(|message| message.contains("MP2 memory limit")));
    }

    #[test]
    fn nickel_contract_errors_keep_field_specific_messages() {
        for (source, expected_message, expected_value) in [
            (
                "[basis]\nname = 4\n",
                "The basis set must be written as a string.",
                "4",
            ),
            (
                "[basis]\nname = \"sto-3g\"\n[method.hf]\nmax_iterations = 0\n",
                "The HF iteration limit must be an integer greater than zero.",
                "0",
            ),
            (
                "[basis]\nname = \"sto-3g\"\n[method.hf]\nconvergence_threshold = 0.0\n",
                "The HF convergence threshold must be a positive finite number.",
                "0.0",
            ),
            (
                "[basis]\nname = \"sto-3g\"\n[method.mp2]\nfrozen_orbitals = \"one\"\n",
                "The MP2 frozen orbital count must be a non-negative integer.",
                "\"one\"",
            ),
            (
                "[basis]\nname = \"sto-3g\"\n[method.mp2]\nmemory_limit = \"0 B\"\n",
                "The MP2 memory limit must be a positive byte size or auto.",
                "\"0 B\"",
            ),
        ] {
            let err = parse_runfile("calculation.toml", source).unwrap_err();
            assert_eq!(err.to_string(), expected_message);
            let label = err.labels().unwrap().next().unwrap();
            let contents = err
                .source_code()
                .unwrap()
                .read_span(label.inner(), 0, 0)
                .unwrap();
            assert!(std::str::from_utf8(contents.data())
                .unwrap()
                .contains(expected_value));
        }
    }
}
