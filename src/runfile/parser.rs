use crate::runfile::RunFile;

use super::diagnostics::FromTomlErrorMietteExt;

#[derive(Debug)]
pub struct ParsedRunFile {
    pub runfile: RunFile,
    /// Scientific options with locations in the original input, not the formatted output.
    pub hf_config: Option<rustiq_core::config::HfConfig>,
    pub mp2_config: Option<rustiq_core::config::Mp2Config>,
    pub molecule_config: rustiq_core::config::MoleculeConfig,
    pub integral_config: rustiq_core::config::IntegralConfig,
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

    let mut hf_config = runfile
        .method
        .hf
        .as_ref()
        .map(rustiq_core::config::HfConfig::from);
    let mut mp2_config = runfile
        .method
        .mp2
        .as_ref()
        .map(rustiq_core::config::Mp2Config::from);
    let mut molecule_config = rustiq_core::config::MoleculeConfig::from(&runfile.molecule);
    let mut integral_config = rustiq_core::config::IntegralConfig::from(&runfile.integrals);

    let root = document.into_item();
    let span = |path: &[&str]| {
        let item = path.iter().try_fold(&root, |item, key| item[*key].item())?;
        let span = item.span();
        Some((span.start as usize, (span.end - span.start) as usize).into())
    };

    if let Some(config) = &mut hf_config {
        config.method.span = span(&["method", "hf", "method"]);
        config.guess.span = span(&["method", "hf", "guess"]);
        config.diis.max_history.span = span(&["method", "hf", "diis", "max_history"]);
        config.orthogonalization.linear_dependency_threshold.span = span(&[
            "method",
            "hf",
            "orthogonalization",
            "linear_dependency_threshold",
        ]);
    }
    if let Some(config) = &mut mp2_config {
        config.frozen_orbitals.span = span(&["method", "mp2", "frozen_orbitals"]);
        config.memory_limit.span = span(&["method", "mp2", "memory_limit"]);
    }
    molecule_config.charge.span = span(&["molecule", "charge"]);
    molecule_config.multiplicity.span = span(&["molecule", "multiplicity"]);
    integral_config.schwarz_threshold.span = span(&["integrals", "schwarz_threshold"]);

    Ok(ParsedRunFile {
        runfile,
        hf_config,
        mp2_config,
        molecule_config,
        integral_config,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_runfile;

    #[test]
    fn parser_preserves_nested_configuration_spans() {
        let source = r#"
[molecule]
charge = 1
multiplicity = 2

[basis]
name = "sto-3g"

[method.hf]
method = "Rhf"

[method.hf.guess]
type = "CoreHamiltonian"

[method.hf.orthogonalization]
linear_dependency_threshold = 1.0

[method.hf.diis]
enabled = true
max_history = 8

[method.mp2]
frozen_orbitals = 1
memory_limit = "auto"

[integrals]
schwarz_threshold = 1e-10
"#;
        let parsed = parse_runfile("calculation.toml", source).unwrap();
        let hf = parsed.hf_config.unwrap();

        let method_span = hf.method.span.unwrap();
        assert_eq!(
            &source[method_span.offset()..method_span.offset() + method_span.len()],
            "\"Rhf\""
        );
        assert!(hf.guess.span.is_some());
        assert!(hf
            .orthogonalization
            .linear_dependency_threshold
            .span
            .is_some());
        assert!(hf.diis.max_history.span.is_some());
        assert!(parsed.integral_config.schwarz_threshold.span.is_some());
        assert!(parsed.molecule_config.charge.span.is_some());
        assert!(parsed.molecule_config.multiplicity.span.is_some());
        let mp2 = parsed.mp2_config.unwrap();
        assert!(mp2.frozen_orbitals.span.is_some());
        assert!(mp2.memory_limit.span.is_some());
    }

    #[test]
    fn omitted_defaults_have_no_source_span() {
        let parsed = parse_runfile(
            "defaults.toml",
            "[molecule]\n[basis]\nname = \"sto-3g\"\n[method.hf]\n[method.mp2]\n",
        )
        .unwrap();
        let hf = parsed.hf_config.unwrap();
        assert!(hf.method.span.is_none());
        assert!(hf.guess.span.is_none());
        assert!(hf.diis.max_history.span.is_none());
        assert!(hf
            .orthogonalization
            .linear_dependency_threshold
            .span
            .is_none());
        assert!(parsed.integral_config.schwarz_threshold.span.is_none());
        let mp2 = parsed.mp2_config.unwrap();
        assert!(mp2.frozen_orbitals.span.is_none());
        assert!(mp2.memory_limit.span.is_none());
    }
}
