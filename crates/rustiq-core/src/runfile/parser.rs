use miette::IntoDiagnostic;

use crate::runfile::RunFile;

use super::diagnostics::FromTomlErrorMietteExt;

#[derive(Debug)]
pub struct ParsedRunFile {
    pub runfile: RunFile,
    pub formatted_toml: String,
    /// Scientific options with locations in the original input, not the formatted output.
    pub hf_config: Option<crate::config::HfConfig>,
    pub mp2_config: Option<crate::config::Mp2Config>,
    pub molecule_config: crate::config::MoleculeConfig,
}

pub fn parse_runfile(
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
        .format(&runfile.output(super::output::Defaults::Include))
        .into_diagnostic()?;

    let mut hf_config = runfile.hf.as_ref().map(crate::config::HfConfig::from);
    let mut molecule_config = crate::config::MoleculeConfig::from(&runfile.global.molecule);
    let mut mp2_config = runfile.mp2.as_ref().map(crate::config::Mp2Config::from);
    let root = document.into_item();
    let span = |section: &str, field: &str| {
        root[section][field].item().map(|item| {
            let span = item.span();
            (span.start as usize, (span.end - span.start) as usize).into()
        })
    };
    if let Some(config) = &mut hf_config {
        config.source_spans.method = span("hf", "method");
        config.source_spans.linear_dependency_threshold = span("hf", "linear_dependency_threshold");
        config.source_spans.guess = span("hf", "guess");
    }
    if let Some(config) = &mut mp2_config {
        config.frozen_orbitals_span = span("mp2", "frozen_orbitals");
    }
    let molecule_span = |field: &str| {
        root["global"]["molecule"][field].item().map(|item| {
            let span = item.span();
            (span.start as usize, (span.end - span.start) as usize).into()
        })
    };
    molecule_config.charge.span = molecule_span("charge");
    molecule_config.multiplicity.span = molecule_span("multiplicity");

    Ok(ParsedRunFile {
        runfile,
        formatted_toml,
        hf_config,
        mp2_config,
        molecule_config,
    })
}
