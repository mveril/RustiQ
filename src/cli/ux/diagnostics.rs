use std::{
    error::Error,
    fmt,
    io::{self, Write},
};

use miette::{Diagnostic, IntoDiagnostic, MietteHandler, Report, ReportHandler};

const FALLBACK_CODE: &str = "rustiq::unexpected_error";

pub(crate) fn code(diagnostic: &dyn Diagnostic) -> String {
    diagnostic
        .code()
        .map_or_else(|| FALLBACK_CODE.to_owned(), |code| code.to_string())
}

/// Give uncoded reports an operation-specific code while retaining their diagnostic details.
pub(crate) fn with_code(report: Report, code: &'static str) -> Report {
    if report.code().is_some() {
        report
    } else {
        Report::new(CodedReport { report, code })
    }
}

#[derive(Debug)]
struct CodedReport {
    report: Report,
    code: &'static str,
}

impl fmt::Display for CodedReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.report, formatter)
    }
}

impl Error for CodedReport {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.report.source()
    }
}

impl Diagnostic for CodedReport {
    fn code(&self) -> Option<Box<dyn fmt::Display + '_>> {
        Some(Box::new(self.code))
    }

    fn severity(&self) -> Option<miette::Severity> {
        self.report.severity()
    }

    fn help(&self) -> Option<Box<dyn fmt::Display + '_>> {
        self.report.help()
    }

    fn url(&self) -> Option<Box<dyn fmt::Display + '_>> {
        self.report.url()
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        self.report.source_code()
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        self.report.labels()
    }

    fn related(&self) -> Option<Box<dyn Iterator<Item = &dyn Diagnostic> + '_>> {
        self.report.related()
    }

    fn diagnostic_source(&self) -> Option<&dyn Diagnostic> {
        self.report.diagnostic_source()
    }
}

pub(crate) struct CodedHandler(pub(crate) MietteHandler);

impl ReportHandler for CodedHandler {
    fn debug(
        &self,
        diagnostic: &dyn Diagnostic,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        if diagnostic.code().is_some() {
            self.0.debug(diagnostic, formatter)
        } else {
            self.0.debug(&UncodedDiagnostic(diagnostic), formatter)
        }
    }
}

#[derive(Debug, Diagnostic)]
#[diagnostic(code("rustiq::unexpected_error"), forward(0))]
struct UncodedDiagnostic<'a>(&'a dyn Diagnostic);

impl fmt::Display for UncodedDiagnostic<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.0, formatter)
    }
}

impl Error for UncodedDiagnostic<'_> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.0.source()
    }
}

/// Emit a structured warning through the global diagnostic hook.
/// Only a failure to write the report makes emission fail.
pub(crate) fn emit_warning(
    diagnostic: impl Diagnostic + Send + Sync + 'static,
) -> miette::Result<()> {
    let report = miette::Report::new(diagnostic);
    writeln!(io::stderr().lock(), "{report:?}").into_diagnostic()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, thiserror::Error, Diagnostic)]
    #[error("original message")]
    #[diagnostic(severity(Warning), help("retry"), url("https://example.com"))]
    struct Details {
        #[source_code]
        source: String,
        #[label("input")]
        span: miette::SourceSpan,
        #[related]
        related: Vec<miette::MietteDiagnostic>,
        #[diagnostic_source]
        diagnostic: miette::MietteDiagnostic,
        #[source]
        cause: io::Error,
    }

    #[test]
    fn wrappers_preserve_diagnostic_properties() {
        fn check(d: &dyn Diagnostic) {
            assert_eq!(d.to_string(), "original message");
            assert_eq!(d.source().unwrap().to_string(), "cause");
            assert_eq!(d.severity(), Some(miette::Severity::Warning));
            assert_eq!(d.help().unwrap().to_string(), "retry");
            assert_eq!(d.url().unwrap().to_string(), "https://example.com");
            assert!(d
                .source_code()
                .unwrap()
                .read_span(&(0, 2).into(), 0, 0)
                .is_ok());
            let label = d.labels().unwrap().next().unwrap();
            assert_eq!(label.label(), Some("input"));
            assert_eq!(label.offset(), 0);
            assert_eq!(label.len(), 2);
            assert_eq!(d.related().unwrap().next().unwrap().to_string(), "related");
            assert_eq!(
                d.diagnostic_source().unwrap().to_string(),
                "diagnostic source"
            );
        }
        let details = Details {
            source: "input".into(),
            span: (0, 2).into(),
            related: vec![miette::MietteDiagnostic::new("related")],
            diagnostic: miette::MietteDiagnostic::new("diagnostic source"),
            cause: io::Error::other("cause"),
        };
        check(&UncodedDiagnostic(&details));
        let report = with_code(Report::new(details), "rustiq::fallback");
        check(report.as_ref());
        assert_eq!(code(report.as_ref()), "rustiq::fallback");
    }

    #[test]
    fn operation_code_preserves_the_original_message_and_cause() {
        let report = Err::<(), _>(io::Error::new(io::ErrorKind::NotFound, "file is missing"))
            .into_diagnostic()
            .unwrap_err()
            .wrap_err("could not read input");
        let report = with_code(report, "rustiq::run::input_read_failed");
        assert_eq!(code(report.as_ref()), "rustiq::run::input_read_failed");
        assert_eq!(
            report.chain().map(ToString::to_string).collect::<Vec<_>>(),
            ["could not read input", "file is missing"]
        );
    }

    #[test]
    fn operation_code_keeps_existing_diagnostics() {
        let report = miette::miette!(code = "rustiq::runfile::nickel", "invalid input");
        let report = with_code(report.wrap_err("calculation failed"), "rustiq::run::failed");
        assert_eq!(code(report.as_ref()), "rustiq::runfile::nickel");
    }
}
