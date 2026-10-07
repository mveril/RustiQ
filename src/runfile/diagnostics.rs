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
    #[related]
    details: Vec<NickelDiagnosticDetail>,
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
        details: Vec::new(),
    }
}

impl NickelRunfileDiagnostic {
    pub(crate) fn with_details(mut self, details: Vec<NickelDiagnosticDetail>) -> Self {
        self.details = details;
        self
    }
}

/// Nickel's own labels and notes, grouped by source file for miette.
#[derive(Debug, Error, Diagnostic)]
#[error("{message}")]
#[diagnostic(code(rustiq::runfile::nickel::detail))]
pub(crate) struct NickelDiagnosticDetail {
    pub(crate) message: String,
    #[source_code]
    pub(crate) source_code: Option<NamedSource<String>>,
    #[label(collection)]
    pub(crate) labels: Vec<miette::LabeledSpan>,
    #[help]
    pub(crate) notes: Option<String>,
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

#[allow(
    clippy::case_sensitive_file_extension_comparisons,
    reason = "These are case-sensitive TOML field paths, not filesystem extensions"
)]
pub(crate) fn humanized_runfile_error(
    kind: super::nickel::ConfigurationErrorKind,
    path: Option<&str>,
    raw_message: &str,
    default_label: &str,
) -> (String, String) {
    let Some(path) = path else {
        return (
            match kind {
                super::nickel::ConfigurationErrorKind::TomlSyntax => {
                    "The runfile is not valid TOML."
                }
                super::nickel::ConfigurationErrorKind::Contract
                | super::nickel::ConfigurationErrorKind::Domain => {
                    "The runfile configuration is invalid."
                }
            }
            .to_string(),
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
    fn native_toml_syntax_errors_preserve_utf8_offsets_and_source_name() {
        let source = "# é\nbasis = { name = 'sto-3g' }\nmethod = { hf = { max_iterations = @ } }\n";
        let error = parse_runfile("stdin.toml", source).unwrap_err();
        let label = error.labels().unwrap().next().unwrap();
        assert_eq!(label.offset(), source.find('@').unwrap());
        assert_eq!(
            source
                .get(label.offset()..label.offset() + label.len())
                .unwrap(),
            "@ "
        );
        let contents = error
            .source_code()
            .unwrap()
            .read_span(label.inner(), 0, 0)
            .unwrap();
        assert_eq!(contents.name(), Some("stdin.toml"));
        let native = error.related().unwrap().next().unwrap();
        assert!(native.labels().is_some());
    }

    #[test]
    fn native_contract_labels_retain_the_original_input_source() {
        let source = "basis = { name = 'sto-3g' }\nmethod = { hf = { max_iterations = 0 } }\n";
        let error = parse_runfile("inline.toml", source).unwrap_err();
        let details = error.related().unwrap().collect::<Vec<_>>();
        assert!(!details.is_empty());
        assert!(details.iter().any(|detail| detail.labels().is_some()));
        assert!(details.iter().any(|detail| {
            detail.labels().is_some_and(|mut labels| {
                labels.any(|label| {
                    detail
                        .source_code()
                        .unwrap()
                        .read_span(label.inner(), 0, 0)
                        .is_ok_and(|contents| contents.name() == Some("inline.toml"))
                })
            })
        }));
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
    fn inline_table_diagnostics_label_the_correct_nested_values() {
        let source = "basis = { name = 'sto-3g' }\nmethod = { hf = { max_iterations = 0, diis = { enabled = 'invalid-diis' } } }\ncache = { enabled = 'invalid-cache' }\n";
        let error = parse_runfile("inline.toml", source).unwrap_err();
        assert_eq!(
            error.to_string(),
            "runfile contains 3 configuration error(s)"
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
                assert_eq!(contents.name(), Some("inline.toml"));
                std::str::from_utf8(contents.data()).unwrap().to_owned()
            })
            .collect::<Vec<_>>();
        values.sort();
        assert_eq!(values, ["'invalid-cache'", "'invalid-diis'", "0"]);
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

    #[test]
    fn non_finite_toml_numbers_are_rejected_with_source_locations() {
        for source in [
            "[method.hf]\nconvergence_threshold = inf\n",
            "[method.hf.orthogonalization]\nlinear_dependency_threshold = nan\n",
            "[integrals]\nschwarz_threshold = inf\n",
            "[method.hf.guess]\ntype = \"Random\"\ndistribution = \"Normal\"\nmean = nan\nstd_dev = inf\n",
            "[method.hf.guess]\ntype = \"Random\"\ndistribution = \"Uniform\"\nmin = -inf\nmax = inf\n",
            "[[unexpected]]\nvalue = inf\n",
        ] {
            let result = std::panic::catch_unwind(|| parse_runfile("non-finite.toml", source));
            let error = result.expect("non-finite TOML must not panic").unwrap_err();
            assert!(error.to_string().contains("finite"));
            assert!(error.to_string().contains("non-finite.toml"));
        }
    }

    #[test]
    fn missing_configuration_fields_do_not_claim_toml_is_malformed() {
        for source in [
            "[molecule]\ngeometry = \"molecule.xyz\"\n",
            "[basis]\n",
            "[basis]\nname = \"sto-3g\"\n[molecule]\ngeometry = \"molecule.xyz\"\n[method]\nhf = 2\n",
        ] {
            let error = parse_runfile("missing.toml", source).unwrap_err();
            assert!(!error.to_string().contains("not valid TOML"));
        }
        let error = parse_runfile("broken.toml", "[molecule\n").unwrap_err();
        assert!(error.to_string().contains("not valid TOML"));
    }
}
