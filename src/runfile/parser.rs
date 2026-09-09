use miette::IntoDiagnostic;

use crate::runfile::RunFile;

use super::diagnostics::FromTomlErrorMietteExt;

#[derive(Debug)]
pub struct ParsedRunFile {
    pub runfile: RunFile,
    pub formatted_toml: String,
    /// Scientific options with locations in the original input, not the formatted output.
    pub hf_config: Option<rustiq_core::config::HfConfig>,
    pub mp2_config: Option<rustiq_core::config::Mp2Config>,
    pub molecule_config: rustiq_core::config::MoleculeConfig,
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

    let mut hf_config = runfile.hf.as_ref().map(rustiq_core::config::HfConfig::from);
    let mut molecule_config = rustiq_core::config::MoleculeConfig::from(&runfile.global.molecule);
    let mut mp2_config = runfile
        .mp2
        .as_ref()
        .map(rustiq_core::config::Mp2Config::from);
    let root = document.into_item();
    let span = |section: &str, field: &str| {
        root[section][field].item().map(|item| {
            let span = item.span();
            (span.start as usize, (span.end - span.start) as usize).into()
        })
    };
    if let Some(config) = &mut hf_config {
        config.method.span = span("hf", "method");
        config.linear_dependency_threshold.span = span("hf", "linear_dependency_threshold");
        config.guess.span = span("hf", "guess");
    }
    if let Some(config) = &mut mp2_config {
        config.frozen_orbitals.span = span("mp2", "frozen_orbitals");
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

#[cfg(test)]
mod tests {
    use approx::assert_abs_diff_eq;
    use miette::{Diagnostic, SourceSpan};
    use rustiq_core::{
        basis::BasisFile, calculation::CalculationExecution, config::HfConfig,
        molecules::geometry::Geometry,
    };

    fn geometry() -> Geometry {
        Geometry::from_reader(&include_bytes!("../../samples/h2/molecule.xyz")[..]).unwrap()
    }

    fn labels(error: &impl Diagnostic) -> Vec<SourceSpan> {
        error
            .labels()
            .into_iter()
            .flatten()
            .map(|label| *label.inner())
            .collect()
    }

    #[test]
    fn toml_adapter_preserves_original_spans_and_numerical_behavior() {
        use crate::runfile::parser::parse_runfile;
        let source = "# original input\n[global]\nbasis = 'sto-3g'\n[global.molecule]\ncharge = 1\nmultiplicity = 2\n[hf]\nmethod = 'Rhf'\nlinear_dependency_threshold = 1.0\n[mp2]\nfrozen_orbitals = 1\n";
        let parsed = parse_runfile("calculation.toml", source).unwrap();
        let molecule = parsed.molecule_config.build(geometry()).unwrap();
        let config = parsed.hf_config.unwrap();
        let error = config.resolve_method(&molecule).unwrap_err();
        let span = labels(&error)[0];
        assert_eq!(&source[span.offset()..span.offset() + span.len()], "'Rhf'");
        assert!(error.source_code().is_none());
        let report = miette::Report::new(error).with_source_code(miette::NamedSource::new(
            "calculation.toml",
            source.to_string(),
        ));
        assert_eq!(
            report
                .source_code()
                .unwrap()
                .read_span(&span, 0, 0)
                .unwrap()
                .name(),
            Some("calculation.toml")
        );

        let basis_file =
            BasisFile::from_reader(&include_bytes!("../../tests/data/sto-3g.json")[..]).unwrap();
        let error = rustiq_core::calculation::CalculationBuilder::new(&geometry(), &basis_file)
            .with_hf(config)
            .execute()
            .err()
            .unwrap();
        let span = labels(&error)[0];
        assert_eq!(&source[span.offset()..span.offset() + span.len()], "1.0");
        let error = rustiq_core::calculation::CalculationBuilder::new(&geometry(), &basis_file)
            .with_mp2(parsed.mp2_config.unwrap())
            .execute()
            .unwrap_err();
        let span = labels(&error)[0];
        assert_eq!(span.offset(), source.rfind('1').unwrap());
        assert_eq!(span.len(), 1);

        let parsed = parse_runfile("defaults.toml", "[global]\nbasis = 'sto-3g'\n[hf]\n").unwrap();
        let from_toml = parsed.hf_config.unwrap();
        assert!(from_toml.method.span.is_none());
        assert!(from_toml.guess.span.is_none());
        assert!(from_toml.linear_dependency_threshold.span.is_none());
        let adapted_result =
            rustiq_core::calculation::CalculationBuilder::new(&geometry(), &basis_file)
                .with_hf(from_toml)
                .execute()
                .unwrap()
                .hf
                .summary()
                .scf
                .clone();
        let direct_result =
            rustiq_core::calculation::CalculationBuilder::new(&geometry(), &basis_file)
                .with_hf(HfConfig::default())
                .execute()
                .unwrap()
                .hf
                .summary()
                .scf
                .clone();
        assert_abs_diff_eq!(
            adapted_result.total_energy,
            direct_result.total_energy,
            epsilon = 1e-10
        );
    }
}
