#[cfg(test)]
use crate::runfile::RunFile;
use miette::IntoDiagnostic;

pub(crate) use super::nickel::resolve_nickel;

#[derive(Debug)]
pub struct ParsedRunFile {
    #[cfg(test)]
    pub runfile: RunFile,
    pub resolved: super::resolved::ResolvedInput,
    /// Scientific options with locations in the original input, not the formatted output.
    pub hf_config: Option<rustiq_core::config::HfConfig>,
    pub mp2_config: Option<rustiq_core::config::Mp2Config>,
    pub molecule_config: rustiq_core::config::MoleculeConfig,
    pub integral_config: rustiq_core::config::IntegralConfig,
}

pub fn parsed_calculation(
    calculation: super::resolved::ResolvedCalculationConfig,
) -> miette::Result<ParsedRunFile> {
    Ok(ParsedRunFile {
        #[cfg(test)]
        runfile: super::RunFile {
            molecule: super::molecule::MoleculeConfig {
                geometry: calculation.molecule.geometry.clone(),
                charge: calculation.molecule.charge,
                multiplicity: calculation.molecule.multiplicity,
                units: calculation.molecule_config().units,
            },
            basis: super::basis::BasisConfig {
                name: calculation.basis.name.clone(),
            },
            method: super::method::MethodConfig {
                hf: Some((&calculation.hf_config().into_diagnostic()?).into()),
                mp2: calculation.mp2_config().as_ref().map(Into::into),
            },
            integrals: (&calculation.integral_config().into_diagnostic()?).into(),
            cache: super::cache::CacheConfig {
                enabled: calculation.cache.enabled,
            },
            output: super::output::OutputConfig {
                scf: match calculation.output.scf {
                    super::resolved::ScfOutput::Normal => super::output::ScfOutput::Normal,
                    super::resolved::ScfOutput::Quiet => super::output::ScfOutput::Quiet,
                },
            },
        },
        hf_config: Some(calculation.hf_config().into_diagnostic()?),
        mp2_config: calculation.mp2_config(),
        molecule_config: calculation.molecule_config(),
        integral_config: calculation.integral_config().into_diagnostic()?,
        resolved: super::resolved::ResolvedInput::new(vec![calculation]).into_diagnostic()?,
    })
}

pub fn parse_runfile(
    source_name: impl Into<String>,
    toml_content: &str,
) -> miette::Result<ParsedRunFile> {
    let source_name = source_name.into();
    let (resolved, source_map) =
        super::nickel::resolve_toml_with_locations(&source_name, toml_content);
    let resolved = resolved.map_err(|errors| {
        let diagnostics = errors
            .into_iter()
            .map(|error| {
                let (path, span) = match &source_map {
                    Some(source_map) => {
                        source_map.error_location(error.path.as_deref(), &error.message)
                    }
                    None => (None, error.span),
                };
                let default_label = error
                    .message
                    .lines()
                    .next()
                    .unwrap_or("invalid configuration");
                let (message, label) = super::diagnostics::humanized_runfile_error(
                    error.kind,
                    path.as_deref(),
                    &error.message,
                    default_label,
                );
                super::diagnostics::nickel_error(&source_name, toml_content, message, label, span)
                    .with_details(error.details)
            })
            .collect();
        super::diagnostics::group_nickel_errors(diagnostics)
    })?;
    let source_map = source_map.expect("successfully parsed input has source locations");
    let calculation = resolved.single_calculation().into_diagnostic()?;
    let mut hf_config = Some(calculation.hf_config().into_diagnostic()?);
    let mut mp2_config = calculation.mp2_config();
    let mut molecule_config = calculation.molecule_config();
    let mut integral_config = calculation.integral_config().into_diagnostic()?;

    #[cfg(test)]
    let runfile = RunFile {
        molecule: super::molecule::MoleculeConfig {
            geometry: calculation.molecule.geometry.clone(),
            charge: molecule_config.charge.value,
            multiplicity: molecule_config.multiplicity.value,
            units: molecule_config.units,
        },
        basis: super::basis::BasisConfig {
            name: calculation.basis.name.clone(),
        },
        method: super::method::MethodConfig {
            hf: source_map
                .span(&["method", "hf"])
                .and_then(|_| hf_config.as_ref().map(Into::into)),
            mp2: mp2_config.as_ref().map(Into::into),
        },
        integrals: (&integral_config).into(),
        cache: super::cache::CacheConfig {
            enabled: calculation.cache.enabled,
        },
        output: super::output::OutputConfig {
            scf: match calculation.output.scf {
                super::resolved::ScfOutput::Normal => super::output::ScfOutput::Normal,
                super::resolved::ScfOutput::Quiet => super::output::ScfOutput::Quiet,
            },
        },
    };
    let span = |path: &[&str]| source_map.span(path);

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
        #[cfg(test)]
        runfile,
        resolved,
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
    fn computed_nickel_configuration_has_no_scientific_spans() {
        let input = super::resolve_nickel(
            "input.ncl",
            r#"
            let iterations = 50 + 50 in {
                basis.name = "sto-3g",
                method.hf.max_iterations = iterations,
                method.hf.method = "Rhf",
                method.mp2 = {},
                molecule.charge = 0,
            }
        "#,
        )
        .unwrap();
        let parsed = super::parsed_calculation(input.calculations()[0].clone()).unwrap();
        let hf = parsed.hf_config.as_ref().unwrap();
        assert!(hf.method.span.is_none());
        assert!(hf.guess.span.is_none());
        assert!(hf.diis.max_history.span.is_none());
        assert!(parsed.molecule_config.charge.span.is_none());
        assert!(parsed.integral_config.schwarz_threshold.span.is_none());
        assert!(parsed
            .mp2_config
            .as_ref()
            .unwrap()
            .frozen_orbitals
            .span
            .is_none());
        let canonical = parsed
            .runfile
            .output(super::super::output::Defaults::Include)
            .render()
            .unwrap();
        let replay = parse_runfile("canonical.toml", &canonical).unwrap();
        assert_eq!(parsed.resolved, replay.resolved);
    }

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
            source
                .get(method_span.offset()..method_span.offset() + method_span.len())
                .unwrap(),
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
