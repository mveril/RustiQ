use std::{
    error::Error,
    fmt,
    io::{self, Write},
};

use miette::{Diagnostic, IntoDiagnostic, MietteHandler, ReportHandler};

const FALLBACK_CODE: &str = "rustiq::unexpected_error";

pub(crate) fn code(diagnostic: &dyn Diagnostic) -> String {
    diagnostic
        .code()
        .map_or_else(|| FALLBACK_CODE.to_owned(), |code| code.to_string())
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
