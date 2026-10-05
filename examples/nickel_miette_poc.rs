//! Isolated Nickel-to-miette adapter; the production frontend remains TOML.
use codespan_reporting::diagnostic::{LabelStyle, Severity as NickelSeverity};
use miette::{Diagnostic, IntoDiagnostic, LabeledSpan, NamedSource, Severity};
use nickel_lang_core::{
    error::IntoDiagnostics,
    eval::cache::CacheImpl,
    files::Files,
    program::{Program, ProgramBuilder},
    serialize::{self, ExportFormat},
};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
#[error("Nickel configuration failed")]
#[diagnostic(code(rustiq::poc::nickel))]
struct NickelReport {
    #[related]
    diagnostics: Vec<NickelDiagnostic>,
}

#[derive(Debug, Error)]
#[error("{message}")]
struct NickelDiagnostic {
    message: String,
    severity: Severity,
    help: Option<String>,
    source_code: Option<NamedSource<String>>,
    labels: Vec<LabeledSpan>,
    sources: Vec<SourceDiagnostic>,
}

impl Diagnostic for NickelDiagnostic {
    fn severity(&self) -> Option<Severity> {
        Some(self.severity)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        self.help
            .as_ref()
            .map(|help| Box::new(help) as Box<dyn std::fmt::Display>)
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        self.source_code
            .as_ref()
            .map(|source| source as &dyn miette::SourceCode)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        (!self.labels.is_empty())
            .then(|| Box::new(self.labels.iter().cloned()) as Box<dyn Iterator<Item = LabeledSpan>>)
    }

    fn related<'a>(&'a self) -> Option<Box<dyn Iterator<Item = &'a dyn Diagnostic> + 'a>> {
        Some(Box::new(
            self.sources.iter().map(|source| source as &dyn Diagnostic),
        ))
    }
}

#[derive(Debug, Error, Diagnostic)]
#[error("{message}")]
struct SourceDiagnostic {
    message: String,
    #[source_code]
    source_code: NamedSource<String>,
    #[label(collection)]
    labels: Vec<LabeledSpan>,
}

fn into_miette(error: nickel_lang_core::error::Error, mut files: Files) -> miette::Report {
    let diagnostics = error
        .into_diagnostics(&mut files)
        .into_iter()
        .map(|diagnostic| {
            // One miette source per related diagnostic preserves locations across
            // imports, contracts, and Nickel's generated evaluation sources.
            let mut sources = Vec::<SourceDiagnostic>::new();
            let mut file_ids = Vec::new();
            for label in diagnostic.labels {
                let index = match file_ids.iter().position(|id| *id == label.file_id) {
                    Some(index) => index,
                    None => {
                        let name = files.name(label.file_id).to_string_lossy().into_owned();
                        file_ids.push(label.file_id);
                        sources.push(SourceDiagnostic {
                            message: name.clone(),
                            source_code: NamedSource::new(
                                name,
                                files.source(label.file_id).to_owned(),
                            ),
                            labels: Vec::new(),
                        });
                        sources.len() - 1
                    }
                };
                let span = (label.range.start, label.range.len());
                let message = (!label.message.is_empty()).then_some(label.message);
                sources[index].labels.push(match label.style {
                    LabelStyle::Primary => LabeledSpan::new_primary_with_span(message, span),
                    LabelStyle::Secondary => LabeledSpan::new_with_span(message, span),
                });
            }
            let primary = sources
                .iter()
                .position(|source| source.labels.iter().any(LabeledSpan::primary))
                .or_else(|| (!sources.is_empty()).then_some(0));
            let (source_code, labels) = match primary {
                Some(index) => {
                    let primary = sources.remove(index);
                    (Some(primary.source_code), primary.labels)
                }
                None => (None, Vec::new()),
            };
            NickelDiagnostic {
                source_code,
                labels,
                message: diagnostic.message,
                severity: match diagnostic.severity {
                    NickelSeverity::Bug | NickelSeverity::Error => Severity::Error,
                    NickelSeverity::Warning => Severity::Warning,
                    NickelSeverity::Note | NickelSeverity::Help => Severity::Advice,
                },
                help: (!diagnostic.notes.is_empty()).then(|| diagnostic.notes.join("\n")),
                sources,
            }
        })
        .collect();
    NickelReport { diagnostics }.into()
}

fn evaluate(path: &Path) -> miette::Result<String> {
    let mut program: Program<CacheImpl> = ProgramBuilder::new()
        .add_path(path.as_os_str())
        .build()
        .into_diagnostic()?;
    let value = program
        .eval_full_for_export()
        .map_err(|error| into_miette(error, program.files()))?;
    serialize::to_string(ExportFormat::Json, &value).map_err(|error| {
        into_miette(
            nickel_lang_core::error::Error::export_error(program.pos_table().clone(), error),
            program.files(),
        )
    })
}

fn main() -> miette::Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/nickel/invalid/multiplicity.ncl")
        });
    println!("{}", evaluate(&path)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use miette::GraphicalReportHandler;

    fn render(report: &miette::Report) -> String {
        let mut rendered = String::new();
        GraphicalReportHandler::new()
            .with_links(false)
            .with_theme(miette::GraphicalTheme::unicode_nocolor())
            .with_context_lines(1)
            .render_report(&mut rendered, report.as_ref())
            .unwrap();
        rendered
    }

    #[test]
    fn renders_parse_error_with_original_utf8_source_span() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("parse.ncl");
        let source = "# café\n{ broken = }";
        std::fs::write(&path, source).unwrap();
        let report = evaluate(&path).unwrap_err();
        let rendered = render(&report);
        assert!(rendered.contains("parse.ncl"), "{rendered}");
        assert!(rendered.contains("broken = }"), "{rendered}");
        let diagnostic = report.related().unwrap().next().unwrap();
        let label = diagnostic.labels().unwrap().next().unwrap();
        let contents = diagnostic
            .source_code()
            .unwrap()
            .read_span(label.inner(), 0, 0)
            .unwrap();
        assert!(contents.name().unwrap().ends_with("parse.ncl"));
        assert_eq!(label.offset(), source.find('}').unwrap());
    }

    #[test]
    fn renders_contract_and_imported_value_in_separate_sources() {
        let dir = tempfile::tempdir().unwrap();
        let contract = dir.path().join("contract.ncl");
        let input = dir.path().join("input.ncl");
        std::fs::write(&contract, "{ count | Number }").unwrap();
        std::fs::write(&input, "{ count = \"wrong\" } | (import \"contract.ncl\")").unwrap();
        let report = evaluate(&input).unwrap_err();
        let rendered = render(&report);
        for expected in ["contract", "contract.ncl", "input.ncl", "wrong", "Number"] {
            assert!(rendered.contains(expected), "{rendered}");
        }
        let diagnostic = report.related().unwrap().next().unwrap();
        assert!(diagnostic.source_code().is_some());
        assert!(diagnostic.related().unwrap().count() >= 1);
    }

    #[test]
    fn renders_every_invalid_poc_fixture_and_preserves_notes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/nickel/invalid");
        let mut count = 0;
        for entry in std::fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|extension| extension != "ncl") {
                continue;
            }
            let report = evaluate(&path).unwrap_err();
            let rendered = render(&report);
            assert!(
                rendered.contains("Nickel configuration failed"),
                "{rendered}"
            );
            assert!(report.related().unwrap().next().is_some());
            if path.file_name().unwrap() == "unknown-field.ncl" {
                assert!(rendered.contains("misspelled"), "{rendered}");
            }
            count += 1;
        }
        assert!(count > 0);
    }

    #[test]
    fn renders_export_error_for_non_data_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("function.ncl");
        std::fs::write(&path, "fun x => x").unwrap();
        let report = evaluate(&path).unwrap_err();
        let rendered = render(&report);
        assert!(rendered.contains("serialization failed"), "{rendered}");
        assert!(rendered.contains("Function"), "{rendered}");
        let diagnostic = report.related().unwrap().next().unwrap();
        assert!(diagnostic.source_code().is_none());
        assert!(diagnostic.labels().is_none());
    }

    #[test]
    fn evaluates_all_valid_poc_inputs_including_toml() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/nickel");
        for name in [
            "single.ncl",
            "multiple.ncl",
            "variants.ncl",
            "import-toml.ncl",
        ] {
            let json = evaluate(&root.join(name)).unwrap();
            let value: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert!(!value["calculations"].as_array().unwrap().is_empty());
        }
    }
}
