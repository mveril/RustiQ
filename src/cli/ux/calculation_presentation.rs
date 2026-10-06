// Full canonical views are lazy capabilities for inspection and artifact reuse;
// the normal `run` path uses only the concise summary below.
#![allow(
    dead_code,
    reason = "Retained scientific helpers and representations support tests, benchmarks, or future internal use"
)]

use std::{
    fmt::Write,
    num::NonZeroUsize,
    path::{Path, PathBuf},
};

use rustiq_core::{
    calculation::{CalculationRequest, PreparedCalculation},
    config::{
        self,
        validated::{DiisSize, NonNegativeFiniteF64, PositiveFiniteF64},
    },
    molecules::{geometry::Geometry, units::Units},
};
use toml_spanner::{ToTomlError, Toml};

use crate::runfile::{
    global::{molecule_config::MoleculeConfig, Global},
    hf::{DensityGuessConfig, HfMethod, HfOutputFormat},
    mp2::Mp2Config,
};

/// CLI rendering schema, not a scientific persistence schema. The generated
/// geometry filename belongs to this TOML/XYZ adapter, never to the core views.
/// Resolved TOML includes the loaded basis label for CLI replay; replay assumes
/// a compatible basis store, while `PreparedCalculation::get_basis()` is the
/// authoritative resolved basis content.
#[derive(Toml)]
#[toml(ToToml)]
struct CalculationToml {
    global: Global,
    hf: HfSettings,
    mp2: Option<Mp2Config>,
    cache: Option<crate::runfile::cache::CacheConfig>,
}

/// Scientific HF settings using the existing runfile spelling and serializers.
/// The canonical runfile includes terminal/reporting options for full replay.
#[derive(Toml)]
#[toml(ToToml)]
struct HfSettings {
    method: HfMethod,
    #[toml(with = crate::runfile::validated::non_zero_usize)]
    max_iterations: NonZeroUsize,
    #[toml(with = crate::runfile::validated::positive_finite_f64)]
    convergence_threshold: PositiveFiniteF64,
    #[toml(with = crate::runfile::validated::non_negative_finite_f64)]
    linear_dependency_threshold: NonNegativeFiniteF64,
    // A disabled threshold must be explicit: omitting it would restore screening.
    eri_schwarz_threshold: f64,
    guess: DensityGuessConfig,
    diis: bool,
    #[toml(with = crate::runfile::validated::diis_size)]
    diis_size: DiisSize,
    format: HfOutputFormat,
}

impl From<&config::HfConfig> for HfSettings {
    fn from(value: &config::HfConfig) -> Self {
        Self {
            method: value.method.value.into(),
            max_iterations: value.max_iterations,
            convergence_threshold: value.convergence_threshold,
            linear_dependency_threshold: value.linear_dependency_threshold.value,
            eri_schwarz_threshold: value
                .eri_schwarz_threshold
                .map_or(0.0, PositiveFiniteF64::into_inner),
            guess: value.guess.value.into(),
            diis: value.diis,
            diis_size: value.diis_size,
            format: HfOutputFormat::default(),
        }
    }
}

impl CalculationToml {
    fn requested(
        request: &CalculationRequest,
        cache_enabled: bool,
        format: HfOutputFormat,
    ) -> Self {
        Self {
            global: Global {
                basis: request.basis_name().into(),
                molecule: MoleculeConfig {
                    geometry: "molecule.xyz".into(),
                    charge: request.molecule().charge.value,
                    multiplicity: request.molecule().multiplicity.value,
                    molecule_unit: request.molecule().units,
                },
            },
            hf: HfSettings {
                format,
                ..request.hf().into()
            },
            mp2: request.mp2().map(Into::into),
            cache: Some(crate::runfile::cache::CacheConfig {
                enabled: cache_enabled,
            }),
        }
    }

    fn resolved(prepared: &PreparedCalculation) -> Self {
        let molecule = prepared.get_molecule();
        Self {
            global: Global {
                basis: prepared.basis_name().into(),
                molecule: MoleculeConfig {
                    geometry: "molecule.xyz".into(),
                    charge: molecule.charge(),
                    multiplicity: molecule.multiplicity(),
                    molecule_unit: molecule.unit(),
                },
            },
            hf: (&prepared.hf_config()).into(),
            mp2: prepared.mp2_config().map(Into::into),
            cache: None,
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
    format: HfOutputFormat,
) -> Result<CanonicalPair, ToTomlError> {
    let toml = render_calculation_toml(CalculationToml::requested(request, cache_enabled, format))?;
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

/// Concise text shown by the normal `run` command. Full canonical renderings
/// remain available through the requested/resolved helpers above and below.
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
        // f64 Display retains enough digits to recover each coordinate exactly.
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
            .with_basis_label(parsed.runfile.global.basis)
            .with_molecule_config(parsed.molecule_config)
            .with_hf(parsed.hf_config.unwrap_or_default())
            .with_mp2(parsed.mp2_config)
            .prepare()
            .unwrap()
    }

    #[test]
    fn source_provenance_keeps_toml_and_xyz_text_exact() {
        let toml = "# Original comment\n[global]\nbasis = 'sto-3g' # Original quote and spacing\n";
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
    fn summary_shows_effective_values_and_only_meaningful_requested_differences() {
        let geometry =
            Geometry::from_source("molecule.xyz", "2\nH2\nH 0 0 -0.37\nH 0 0 0.37\n").unwrap();
        let prepared = prepare("[global]\nbasis = 'sto-3g'\n", &geometry);
        let summary = calculation_summary(&prepared, Path::new("../molecule.xyz"));

        assert!(summary.contains("Geometry      ../molecule.xyz"));
        assert!(summary.contains("Atoms         2"));
        assert!(summary.contains("Charge        0"));
        assert!(summary.contains("Multiplicity  1"));
        assert!(summary.contains("Coordinates   Bohr (input: Angstrom)"));
        assert!(summary.contains("HF            RHF (requested: Auto)"));
        assert!(summary.contains("Basis         STO-3G (2 functions)"));
        assert!(!summary.contains("requested: 0"));
        assert!(!summary.contains("requested: sto-3g"));
        assert!(!summary.contains("H 0 0"));
    }

    #[test]
    fn copied_canonical_requested_and_resolved_pairs_run_equivalently() {
        let geometry =
            Geometry::from_source("input.xyz", "2\nH2\nH 0 0 -0.37\nH 0 0 0.37\n").unwrap();
        let prepared = prepare("[global]\nbasis = 'sto-3g'\n", &geometry);
        let requested =
            requested_calculation(prepared.request(), false, HfOutputFormat::default()).unwrap();
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
                .with_basis_label(parsed.runfile.global.basis)
                .with_molecule_config(parsed.molecule_config)
                .with_hf(parsed.hf_config.unwrap_or_default())
                .with_mp2(parsed.mp2_config)
                .prepare()
                .unwrap();
            let replay_energy = replay.execute().unwrap().hf.summary().scf.total_energy;
            assert_abs_diff_eq!(replay_energy, original_energy, epsilon = 1e-10);
        }
    }

    #[test]
    fn canonical_views_render_without_source_files_and_preserve_coordinate_precision() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("molecule.xyz");
        std::fs::write(
            &path,
            "2\nOriginal comment\nH 0 0 -0.370000000123456789\nH 0 0 0.370000000123456789\n",
        )
        .unwrap();
        let geometry = Geometry::from_path(&path).unwrap();
        let prepared = prepare("[global]\nbasis = 'sto-3g'\n", &geometry);
        std::fs::remove_file(path).unwrap();
        drop(geometry);

        assert_eq!(prepared.request().molecule().units, Units::Angstrom);
        assert_eq!(prepared.get_molecule().unit(), Units::Bohr);
        assert_eq!(prepared.request().hf().method.value, config::HfMethod::Auto);
        assert_eq!(prepared.hf_config().method.value, config::HfMethod::Rhf);
        assert_eq!(prepared.request().basis_name(), "sto-3g");
        assert_eq!(prepared.basis_name(), "STO-3G");
        for (rendered, expected, method) in [
            (
                CalculationToml::requested(prepared.request(), false, HfOutputFormat::default()),
                prepared.request().geometry(),
                config::HfMethod::Auto,
            ),
            (
                CalculationToml::resolved(&prepared),
                prepared.get_molecule().geometry(),
                config::HfMethod::Rhf,
            ),
        ] {
            let has_cache = rendered.cache.is_some();
            let toml = render_calculation_toml(rendered).unwrap();
            let restored = parse_runfile("rendered.toml", &toml).unwrap();
            assert_eq!(restored.hf_config.unwrap().method.value, method);
            assert!(toml.contains("format = \"Normal\""));
            assert_eq!(toml.contains("[cache]"), has_cache);
            let xyz = geometry_xyz(expected, "Canonical geometry");
            let restored = Geometry::from_source("rendered.xyz", &xyz).unwrap();
            for (actual, expected) in restored.atoms.iter().zip(&expected.atoms) {
                assert_abs_diff_eq!(actual.position, expected.position, epsilon = 1e-14);
                assert_eq!(actual.element.symbol, expected.element.symbol);
            }
        }
        assert!(
            requested_calculation(prepared.request(), false, HfOutputFormat::default())
                .unwrap()
                .toml
                .contains("method = \"Auto\"")
        );
        assert!(resolved_calculation(&prepared)
            .unwrap()
            .configuration
            .contains("method = \"Rhf\""));
    }

    #[test]
    fn requested_canonical_toml_includes_cache_setting() {
        let geometry = Geometry::from_source("input.xyz", "2\nH2\nH 0 0 0\nH 0 0 0.74\n").unwrap();
        let prepared = prepare("[global]\nbasis = 'sto-3g'\n", &geometry);
        let rendered = requested_calculation(prepared.request(), true, HfOutputFormat::default())
            .unwrap()
            .toml;
        assert!(rendered.contains("[cache]\nenabled = true"));
        let restored = parse_runfile("rendered.toml", &rendered).unwrap();
        assert!(restored.runfile.cache.enabled);
    }

    #[test]
    fn equivalent_defaults_and_source_provenance_render_identically() {
        let original = Geometry::from_source(
            "original.xyz",
            "2\nOriginal comment\nH 0 0 -0.37\nH 0 0 0.37\n",
        )
        .unwrap();
        let equivalent = Geometry::from_source(
            "different.xyz",
            "2\nDifferent comment\n1 0.0 0.0 -0.370000\nH 0.0 0.0 0.370000\n",
        )
        .unwrap();
        for post_hf in ["", "\n[mp2]\n"] {
            let implicit = prepare(
                &format!("# omitted defaults\n[global]\nbasis = 'sto-3g'\n{post_hf}"),
                &original,
            );
            let explicit_source = render_calculation_toml(CalculationToml::requested(
                implicit.request(),
                true,
                HfOutputFormat::default(),
            ))
            .unwrap()
            .replace("molecule.xyz", "../different.xyz");
            let explicit_source = format!("# explicitly written defaults\n{explicit_source}");
            let explicit = prepare(&explicit_source, &equivalent);
            assert_eq!(
                requested_calculation(implicit.request(), false, HfOutputFormat::default())
                    .unwrap()
                    .combined(),
                requested_calculation(explicit.request(), false, HfOutputFormat::default())
                    .unwrap()
                    .combined()
            );
            assert_eq!(
                resolved_calculation(&implicit).unwrap().combined(),
                resolved_calculation(&explicit).unwrap().combined()
            );
            assert_eq!(explicit.request().geometry().comment, "");
            assert_eq!(explicit.get_molecule().geometry().comment, "");
            assert!(explicit.request().molecule().charge.span.is_none());
            assert!(explicit.request().molecule().multiplicity.span.is_none());
            for hf in [explicit.request().hf().clone(), explicit.hf_config()] {
                assert!(hf.method.span.is_none());
                assert!(hf.guess.span.is_none());
                assert!(hf.linear_dependency_threshold.span.is_none());
            }
            if let Some(mp2) = explicit.mp2_config() {
                assert!(mp2.frozen_orbitals.span.is_none());
                assert!(mp2.memory_limit.span.is_none());
            }
        }
    }

    #[test]
    fn canonical_request_round_trips_non_default_hf_and_mp2_options() {
        let geometry = Geometry::from_source("input.xyz", "2\nH2\nH 0 0 0\nH 0 0 0.74\n").unwrap();
        for guess in [
            "[hf.guess]\ntype = 'Zero'\n",
            "[hf.guess]\ntype = 'CoreHamiltonian'\n[hf.guess.perturbation]\nseed = 41\n",
            "[hf.guess]\ntype = 'OneElectron'\n[hf.guess.perturbation]\ndistribution = 'Uniform'\nmin = -0.001\nmax = 0.001\nseed = 42\n",
            "[hf.guess]\ntype = 'Random'\ndistribution = 'Normal'\nmean = 0.25\nstd_dev = 0.01\nseed = 43\n",
        ] {
            let source = format!("[global]\nbasis = 'sto-3g'\n[hf]\nmethod = 'Uhf'\nmax_iterations = 42\nconvergence_threshold = 1e-7\nlinear_dependency_threshold = 0.0\neri_schwarz_threshold = 0.0\ndiis = true\ndiis_size = 8\nformat = 'Nope'\n{guess}\n[mp2]\nfrozen_orbitals = 1\nmemory_limit = '9007199254740993 B'\n");
            let prepared = prepare(&source, &geometry);
            let rendered = render_calculation_toml(CalculationToml::requested(prepared.request(), false, HfOutputFormat::Nope)).unwrap();
            let original = parse_runfile("original.toml", &source).unwrap();
            let reparsed = parse_runfile("rendered.toml", &rendered).unwrap();
            assert_eq!(
                toml_spanner::to_string(&original.runfile.hf.unwrap().guess).unwrap(),
                toml_spanner::to_string(&reparsed.runfile.hf.unwrap().guess).unwrap(),
            );
            let restored = prepare(&rendered, &geometry);
            assert_eq!(
                requested_calculation(prepared.request(), false, HfOutputFormat::default()).unwrap().combined(),
                requested_calculation(restored.request(), false, HfOutputFormat::default()).unwrap().combined()
            );
            assert_eq!(prepared.request().hf().max_iterations.get(), 42);
            assert!(prepared.request().hf().eri_schwarz_threshold.is_none());
            assert_eq!(prepared.request().mp2().unwrap().frozen_orbitals.value, 1);
            assert_eq!(prepared.request().mp2().unwrap().memory_limit.value, config::MemoryLimit::Fixed(bytesize::ByteSize::b(9_007_199_254_740_993)));
            assert!(rendered.contains("format = \"Nope\""));
        }
    }

    #[test]
    fn resolved_rendered_toml_includes_prepared_random_seed() {
        let geometry = Geometry::from_source("input.xyz", "2\nH2\nH 0 0 0\nH 0 0 0.74\n").unwrap();
        let prepared = prepare(
            "[global]\nbasis = 'sto-3g'\n[hf.guess]\ntype = 'Random'\ndistribution = 'Uniform'\nmin = -1.0\nmax = 1.0\n",
            &geometry,
        );
        let seed = match prepared.hf_config().guess.value {
            config::DensityGuessConfig::Random { config } => {
                config.random.seed.expect("preparation resolves the seed")
            }
            _ => panic!("expected random guess"),
        };
        let requested = requested_calculation(prepared.request(), false, HfOutputFormat::default())
            .unwrap()
            .toml;
        let resolved = resolved_calculation(&prepared).unwrap().configuration;
        assert!(!requested.contains("seed ="));
        assert!(resolved.contains(&format!("seed = {seed}")));
    }

    #[test]
    fn auto_remains_requested_and_resolves_to_uhf_for_a_doublet() {
        let geometry = Geometry::from_source("input.xyz", "2\nH2+\nH 0 0 0\nH 0 0 1.4\n").unwrap();
        let prepared = prepare("[global]\nbasis = 'sto-3g'\n[global.molecule]\ncharge = 1\nmultiplicity = 2\nmolecule_unit = 'Bohr'\n", &geometry);
        assert_eq!(prepared.request().hf().method.value, config::HfMethod::Auto);
        assert_eq!(prepared.hf_config().method.value, config::HfMethod::Uhf);
        assert!(
            requested_calculation(prepared.request(), false, HfOutputFormat::default())
                .unwrap()
                .toml
                .contains("method = \"Auto\"")
        );
        assert!(resolved_calculation(&prepared)
            .unwrap()
            .configuration
            .contains("method = \"Uhf\""));
    }
}
