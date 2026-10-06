// Full canonical views are lazy capabilities for inspection and artifact reuse;
// the normal `run` path uses only the concise summary below.
#![allow(
    dead_code,
    reason = "Retained scientific helpers and representations support tests, benchmarks, or future internal use"
)]

use std::{
    fmt::Write,
    path::{Path, PathBuf},
};

use rustiq_core::{
    calculation::{CalculationRequest, PreparedCalculation},
    config,
    molecules::{geometry::Geometry, units::Units},
};
use toml_spanner::{ToTomlError, Toml};

use crate::runfile::{
    basis::BasisConfig,
    cache::CacheConfig,
    integrals::IntegralConfig,
    method::MethodConfig,
    molecule::MoleculeConfig,
    output::{OutputConfig, ScfOutput},
};

/// CLI rendering schema, not a scientific persistence schema. The generated
/// geometry filename belongs to this TOML/XYZ adapter, never to the core views.
#[derive(Toml)]
#[toml(ToToml)]
struct CalculationToml {
    #[toml(style = Header)]
    molecule: MoleculeConfig,
    #[toml(style = Header)]
    basis: BasisConfig,
    #[toml(style = Implicit)]
    method: MethodConfig,
    #[toml(style = Header)]
    integrals: IntegralConfig,
    #[toml(style = Header)]
    cache: Option<CacheConfig>,
    #[toml(style = Header)]
    output: OutputConfig,
}

impl CalculationToml {
    fn requested(request: &CalculationRequest, cache_enabled: bool, scf_output: ScfOutput) -> Self {
        Self {
            molecule: MoleculeConfig {
                geometry: "molecule.xyz".into(),
                charge: request.molecule().charge.value,
                multiplicity: request.molecule().multiplicity.value,
                units: request.molecule().units,
            },
            basis: BasisConfig {
                name: request.basis_name().into(),
            },
            method: MethodConfig {
                hf: Some(request.hf().into()),
                mp2: request.mp2().map(Into::into),
            },
            integrals: request.integrals().into(),
            cache: Some(CacheConfig {
                enabled: cache_enabled,
            }),
            output: OutputConfig { scf: scf_output },
        }
    }

    fn resolved(prepared: &PreparedCalculation) -> Self {
        let molecule = prepared.get_molecule();
        Self {
            molecule: MoleculeConfig {
                geometry: "molecule.xyz".into(),
                charge: molecule.charge(),
                multiplicity: molecule.multiplicity(),
                units: molecule.unit(),
            },
            basis: BasisConfig {
                name: prepared.basis_name().into(),
            },
            method: MethodConfig {
                hf: Some((&prepared.hf_config()).into()),
                mp2: prepared.mp2_config().map(Into::into),
            },
            integrals: (&prepared.integral_config()).into(),
            cache: None,
            output: OutputConfig::default(),
        }
    }
}

pub(crate) struct CanonicalPair {
    pub(crate) toml: String,
    pub(crate) xyz: String,
    pub(crate) units: &'static str,
}

/// Exact frontend input retained at the CLI boundary for diagnostics or an
/// explicit source view. It is provenance only and never enters core state.
pub(crate) struct SourceProvenance {
    pub(crate) calculation_name: String,
    pub(crate) calculation: String,
    pub(crate) geometry_path: PathBuf,
    pub(crate) geometry: String,
}

impl SourceProvenance {
    pub(crate) fn new(
        calculation_name: String,
        calculation: String,
        geometry_path: PathBuf,
        geometry: String,
    ) -> Self {
        Self {
            calculation_name,
            calculation,
            geometry_path,
            geometry,
        }
    }
}

impl CanonicalPair {
    #[cfg(test)]
    fn combined(&self) -> String {
        format!(
            "Requested calculation (canonical TOML)\n{}\nRequested geometry (canonical XYZ, {})\n{}",
            self.toml, self.units, self.xyz
        )
    }
}

pub(crate) fn requested_calculation(
    request: &CalculationRequest,
    cache_enabled: bool,
    scf_output: ScfOutput,
) -> Result<CanonicalPair, ToTomlError> {
    let toml = render_calculation_toml(CalculationToml::requested(
        request,
        cache_enabled,
        scf_output,
    ))?;
    Ok(CanonicalPair {
        toml,
        xyz: geometry_xyz(request.geometry(), "Requested geometry"),
        units: unit_name(request.molecule().units),
    })
}

fn render_calculation_toml(configuration: CalculationToml) -> Result<String, ToTomlError> {
    let cache = configuration.cache;
    let mut toml = toml_spanner::to_string(&CalculationToml {
        cache: None,
        ..configuration
    })?;
    if let Some(cache) = cache {
        let cache_toml = toml_spanner::to_string(&cache)?;
        toml.push_str("\n[cache]\n");
        toml.push_str(&cache_toml);
    }
    Ok(toml)
}

pub(crate) struct ResolvedCalculation {
    pub(crate) summary: String,
    pub(crate) configuration: String,
    pub(crate) geometry: String,
}

pub(crate) fn calculation_summary(prepared: &PreparedCalculation, geometry_path: &Path) -> String {
    let request = prepared.request();
    let molecule = prepared.get_molecule();
    let requested_method = match request.hf().method.value {
        config::HfMethod::Auto => "Auto",
        config::HfMethod::Rhf => "RHF",
        config::HfMethod::Uhf => "UHF",
    };
    let method = prepared.hf_method().to_string();
    let method = if requested_method == "Auto" {
        format!("{method} (requested: {requested_method})")
    } else {
        method
    };
    let coordinates = match request.molecule().units {
        Units::Angstrom => "Bohr (input: Angstrom)",
        Units::Bohr => "Bohr",
    };

    format!(
        "Calculation\n  Geometry      {}\n  Atoms         {}\n  Charge        {}\n  Multiplicity  {}\n  Coordinates   {coordinates}\n  HF            {method}\n  Basis         {} ({} functions)",
        geometry_path.display(),
        molecule.geometry().atoms.len(),
        molecule.charge(),
        molecule.multiplicity().get(),
        prepared.basis_name(),
        prepared.get_basis().nbasis(),
    )
}

impl ResolvedCalculation {
    #[cfg(test)]
    fn combined(&self) -> String {
        format!(
            "{}\n\nResolved configuration (canonical TOML)\n{}\nResolved geometry (canonical XYZ, Bohr)\n{}",
            self.summary, self.configuration, self.geometry
        )
    }
}

pub(crate) fn resolved_calculation(
    prepared: &PreparedCalculation,
) -> Result<ResolvedCalculation, ToTomlError> {
    let molecule = prepared.get_molecule();
    Ok(ResolvedCalculation {
        summary: format!(
            "Resolved calculation\n  Coordinates  Bohr\n  Charge       {}\n  Multiplicity {}\n  HF method    {}\n  Basis        {} ({} functions)",
            molecule.charge(),
            molecule.multiplicity().get(),
            prepared.hf_method(),
            prepared.basis_name(),
            prepared.get_basis().nbasis(),
        ),
        configuration: render_calculation_toml(CalculationToml::resolved(prepared))?,
        geometry: geometry_xyz(molecule.geometry(), "Resolved geometry"),
    })
}

fn unit_name(unit: Units) -> &'static str {
    match unit {
        Units::Bohr => "Bohr",
        Units::Angstrom => "Angstrom",
    }
}

fn geometry_xyz(geometry: &Geometry, comment: &str) -> String {
    let mut output = format!("{}\n{comment}\n", geometry.atoms.len());
    for atom in &geometry.atoms {
        writeln!(
            output,
            "{} {} {} {}",
            atom.element.symbol, atom.position.x, atom.position.y, atom.position.z,
        )
        .expect("writing to a String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runfile::parser::parse_runfile;
    use approx::assert_abs_diff_eq;
    use rustiq_core::{
        basis::BasisFile,
        calculation::{CalculationBuilder, CalculationExecution},
    };

    fn basis() -> BasisFile {
        BasisFile::from_reader(&include_bytes!("../../../tests/data/sto-3g.json")[..]).unwrap()
    }

    fn prepare(source: &str, geometry: &Geometry) -> PreparedCalculation {
        let parsed = parse_runfile("input.toml", source).unwrap();
        let basis = basis();
        CalculationBuilder::new(geometry, &basis)
            .with_basis_label(parsed.runfile.basis.name)
            .with_molecule_config(parsed.molecule_config)
            .with_integrals(parsed.integral_config)
            .with_hf(parsed.hf_config.unwrap_or_default())
            .with_mp2(parsed.mp2_config)
            .prepare()
            .unwrap()
    }

    #[test]
    fn source_provenance_keeps_toml_and_xyz_text_exact() {
        let toml = "# Original comment\n[molecule]\n[basis]\nname = 'sto-3g'\n";
        let xyz =
            "2\nGeometry comment\nH  0 0 -0.370000000123456789\nH 0 0   0.370000000123456789\n";
        let source = SourceProvenance::new(
            "calculation.toml".into(),
            toml.into(),
            "../molecule.xyz".into(),
            xyz.into(),
        );

        assert_eq!(source.calculation, toml);
        assert_eq!(source.geometry, xyz);
        assert_eq!(source.calculation_name, "calculation.toml");
        assert_eq!(source.geometry_path, Path::new("../molecule.xyz"));
    }

    #[test]
    fn canonical_requested_and_resolved_pairs_replay_equivalently() {
        let geometry =
            Geometry::from_source("input.xyz", "2\nH2\nH 0 0 -0.37\nH 0 0 0.37\n").unwrap();
        let source = "[molecule]\n[basis]\nname = 'sto-3g'\n[method.hf]\n";
        let prepared = prepare(source, &geometry);
        let requested =
            requested_calculation(prepared.request(), false, ScfOutput::default()).unwrap();
        let resolved = resolved_calculation(&prepared).unwrap();
        let temp = tempfile::tempdir().unwrap();

        let original_energy = prepared.execute().unwrap().hf.summary().scf.total_energy;
        for (name, toml, xyz) in [
            ("requested", requested.toml, requested.xyz),
            ("resolved", resolved.configuration, resolved.geometry),
        ] {
            let directory = temp.path().join(name);
            std::fs::create_dir(&directory).unwrap();
            let toml_path = directory.join("calculation.toml");
            let xyz_path = directory.join("molecule.xyz");
            std::fs::write(&toml_path, toml).unwrap();
            std::fs::write(&xyz_path, xyz).unwrap();

            let input = std::fs::read_to_string(&toml_path).unwrap();
            let parsed = parse_runfile(toml_path.display().to_string(), &input).unwrap();
            let replay_geometry = Geometry::from_path(&xyz_path).unwrap();
            let basis = basis();
            let replay = CalculationBuilder::new(&replay_geometry, &basis)
                .with_basis_label(parsed.runfile.basis.name)
                .with_molecule_config(parsed.molecule_config)
                .with_integrals(parsed.integral_config)
                .with_hf(parsed.hf_config.unwrap_or_default())
                .with_mp2(parsed.mp2_config)
                .prepare()
                .unwrap();
            let replay_energy = replay.execute().unwrap().hf.summary().scf.total_energy;
            assert_abs_diff_eq!(replay_energy, original_energy, epsilon = 1e-10);
        }
    }

    #[test]
    fn canonical_toml_keeps_component_sections_separate() {
        let geometry = Geometry::from_source("input.xyz", "2\nH2\nH 0 0 0\nH 0 0 0.74\n").unwrap();
        let prepared = prepare(
            "[molecule]\n[basis]\nname = 'sto-3g'\n[method.hf]\n[method.hf.diis]\nenabled = true\nmax_history = 8\n[integrals]\nschwarz_threshold = 0.0\n",
            &geometry,
        );
        let rendered = requested_calculation(prepared.request(), true, ScfOutput::Quiet)
            .unwrap()
            .toml;

        assert!(rendered.contains("[method.hf.diis]"));
        assert!(rendered.contains("max_history = 8"));
        assert!(rendered.contains("[method.hf.orthogonalization]"));
        assert!(rendered.contains("[integrals]"));
        assert!(rendered.contains("schwarz_threshold = 0"));
        assert!(rendered.contains("[output]"));
        assert!(rendered.contains("scf = \"Quiet\""));
        assert!(rendered.contains("[cache]\nenabled = true"));
        let replay = parse_runfile("canonical.toml", &rendered).unwrap();
        assert!(replay.integral_config.schwarz_threshold.value.is_none());
        assert!(replay.runfile.cache.enabled);
        assert_eq!(replay.runfile.output.scf, ScfOutput::Quiet);
    }
}
