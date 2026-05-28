use miette::IntoDiagnostic;

use crate::runfile::RunFile;

use super::diagnostics::FromTomlErrorMietteExt;

#[derive(Debug)]
pub(crate) struct ParsedRunFile {
    pub(crate) runfile: RunFile,
    pub(crate) formatted_toml: String,
}

pub(crate) fn parse_runfile(
    source_name: impl Into<String>,
    toml_content: &str,
) -> miette::Result<ParsedRunFile> {
    let source_name = source_name.into();
    let arena = toml_spanner::Arena::new();
    let mut document = toml_spanner::parse(toml_content, &arena)
        .map_err(toml_spanner::FromTomlError::from)
        .map_err(|error| error.into_miette_diagnostic(source_name.clone(), toml_content))?;
    let runfile = document
        .to::<RunFile>()
        .map_err(|error| error.into_miette_diagnostic(source_name, toml_content))?;
    let formatted_toml = toml_spanner::Formatting::preserved_from(&document)
        .format(&runfile)
        .into_diagnostic()?;

    Ok(ParsedRunFile {
        runfile,
        formatted_toml,
    })
}
