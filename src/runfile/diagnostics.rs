use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

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

pub(crate) trait FromTomlErrorMietteExt {
    fn into_miette_diagnostic(self, source_name: String, toml_content: &str) -> miette::Report;
}

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
                    .map(|path| path.to_string())
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

fn source_span(span: toml_spanner::Span) -> SourceSpan {
    let start = span.start as usize;
    let end = span.end as usize;
    (start, end.saturating_sub(start)).into()
}

fn path_for_span(toml_content: &str, span: toml_spanner::Span) -> Option<String> {
    let offset = span.start as usize;
    let line_start = toml_content[..offset.min(toml_content.len())]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    let line_end = toml_content[offset.min(toml_content.len())..]
        .find('\n')
        .map_or(toml_content.len(), |index| offset + index);
    let line = toml_content[line_start..line_end].trim();
    let key = line.split_once('=')?.0.trim();
    if key.is_empty() {
        return None;
    }

    let section = toml_content[..line_start].lines().rev().find_map(|line| {
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

fn humanized_runfile_error(
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
        "global.basis" => (
            "The basis set must be written as a string.".to_string(),
            "expected a basis set name, for example basis = \"sto-3g\"".to_string(),
        ),
        "global.molecule.geometry" => (
            "The molecule geometry path must be a non-empty string.".to_string(),
            "expected a geometry file path".to_string(),
        ),
        "global.molecule.charge" => (
            "The molecule charge must be an integer.".to_string(),
            "expected an integer charge".to_string(),
        ),
        "global.molecule.multiplicity" => (
            "The molecule multiplicity must be an integer greater than zero.".to_string(),
            "expected a positive spin multiplicity".to_string(),
        ),
        "global.molecule.molecule_unit" => (
            "The molecule unit must be one of the supported unit names.".to_string(),
            "expected Bohr or Angstrom".to_string(),
        ),
        "hf.max_iterations" => (
            "The HF iteration limit must be an integer greater than zero.".to_string(),
            "expected a positive iteration count".to_string(),
        ),
        "hf.convergence_threshold" => (
            "The HF convergence threshold must be a positive finite number.".to_string(),
            "expected a positive finite threshold".to_string(),
        ),
        "hf.diis" => (
            "The DIIS flag must be a boolean.".to_string(),
            "expected true or false".to_string(),
        ),
        "hf.diis_size" => (
            "The DIIS history size must be an integer greater than or equal to 2.".to_string(),
            "expected a DIIS history size of at least 2".to_string(),
        ),
        "hf.format" => (
            "The HF output format must be one of the supported format names.".to_string(),
            "expected Normal or Nope".to_string(),
        ),
        "mp2.frozen_orbitals" => (
            "The MP2 frozen orbital count must be a non-negative integer.".to_string(),
            "expected a count of frozen orbitals".to_string(),
        ),
        "hf.guess" => (
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
    fn test_from_toml_error_reports_toml_span() {
        let result = parse_runfile("calculation.toml", "hf = \"not a table\"");

        let err = result.unwrap_err();
        assert!(format!("{err:?}").contains("rustiq::runfile::toml_deserialize"));
    }

    #[test]
    fn test_from_toml_error_reports_multiple_deserialization_errors() {
        let result = parse_runfile(
            "calculation.toml",
            r#"
            [global]
            basis = 4

            [hf]
            max_iterations = 0
            convergence_threshold = 0.0

            [mp2]
            frozen_orbitals = "one"
            "#,
        );

        let err = result.unwrap_err();
        let rendered = format!("{err:?}");
        assert!(rendered.contains("runfile contains 4 configuration error(s)"));
        assert!(rendered.contains("The basis set must be written as a string."));
        assert!(rendered.contains("The HF iteration limit must be an integer greater than zero."));
        assert!(rendered.contains("The HF convergence threshold must be a positive finite number."));
        assert!(rendered.contains("The MP2 frozen orbital count must be a non-negative integer."));
    }

    #[test]
    fn test_from_toml_error_reports_spanned_field_values() {
        let result = parse_runfile(
            "calculation.toml",
            r#"
            [global]
            basis = 4

            [hf]
            max_iterations = 0
            convergence_threshold = 0.0
            "#,
        );

        let err = result.unwrap_err();
        let rendered = format!("{err:?}");
        assert!(rendered.contains("basis = 4"));
        assert!(rendered.contains("max_iterations = 0"));
        assert!(rendered.contains("convergence_threshold = 0.0"));
    }
}
