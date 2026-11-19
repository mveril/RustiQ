use std::io::{self, Write};

use miette::{Diagnostic, IntoDiagnostic};

/// Emit a structured warning through the global diagnostic hook.
/// Only a failure to write the report makes emission fail.
pub(crate) fn emit_warning(
    diagnostic: impl Diagnostic + Send + Sync + 'static,
) -> miette::Result<()> {
    let report = miette::Report::new(diagnostic);
    writeln!(io::stderr().lock(), "{report:?}").into_diagnostic()
}
