//! Migration baseline for the production frontend, independent of rendering and source spans.
// Component tests in parser, hf, mp2, random_config, output, and diagnostics
// already protect spans, validation, distributions, memory syntax, and round trips.
// This baseline adds the missing sample-wide assertions on production core conversion.
use std::{fs, path::Path};

use rustiq_core::{
    config::{DensityGuessConfig, HfMethod, MemoryLimit},
    molecules::units::Units,
};

use super::{output::ScfOutput, parser::parse_runfile};

fn collect_toml_files(dir: &Path, files: &mut Vec<String>, root: &Path) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_toml_files(&path, files, root);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "toml")
        {
            files.push(
                path.strip_prefix(root)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned(),
            );
        }
    }
}

type SampleExpectation = (
    &'static str,
    &'static str,
    &'static str,
    HfMethod,
    i32,
    u8,
    usize,
    f64,
    Option<usize>,
    bool,
    bool,
);

const SAMPLE_EXPECTATIONS: &[SampleExpectation] = &[
    (
        "anthracene/cc-pvdz/calculation.toml",
        "../anthracene.xyz",
        "cc-pvdz",
        HfMethod::Auto,
        0,
        1,
        300,
        1e-8,
        Some(6),
        true,
        true,
    ),
    (
        "benzene/cc-pvdz/calculation.toml",
        "../benzene.xyz",
        "cc-pvdz",
        HfMethod::Auto,
        0,
        1,
        150,
        1e-10,
        Some(10),
        false,
        false,
    ),
    (
        "cholesterol/sto-3g/calculation.toml",
        "../cholesterol.xyz",
        "sto-3g",
        HfMethod::Auto,
        0,
        1,
        100,
        1e-8,
        Some(8),
        true,
        false,
    ),
    (
        "ethanol/6-31g/calculation.toml",
        "../ethanol.xyz",
        "6-31g",
        HfMethod::Auto,
        0,
        1,
        100,
        1e-10,
        Some(8),
        false,
        false,
    ),
    (
        "ethanol/cc-pvdz/calculation.toml",
        "../ethanol.xyz",
        "cc-pvdz",
        HfMethod::Auto,
        0,
        1,
        120,
        1e-10,
        Some(10),
        false,
        false,
    ),
    (
        "ethanol/sto-3g/calculation.toml",
        "../ethanol.xyz",
        "sto-3g",
        HfMethod::Auto,
        0,
        1,
        80,
        1e-10,
        Some(8),
        false,
        false,
    ),
    (
        "h2/6-31g/calculation.toml",
        "../molecule.xyz",
        "6-31g",
        HfMethod::Auto,
        0,
        1,
        100,
        1e-8,
        Some(2),
        false,
        false,
    ),
    (
        "h2/cc-pvdz/calculation.toml",
        "../molecule.xyz",
        "cc-pvdz",
        HfMethod::Auto,
        0,
        1,
        100,
        1e-8,
        Some(8),
        false,
        false,
    ),
    (
        "h2/cc-pvdz/mp2_calculation.toml",
        "../molecule.xyz",
        "cc-pvdz",
        HfMethod::Rhf,
        0,
        1,
        80,
        1e-12,
        Some(8),
        true,
        false,
    ),
    (
        "h2/sto-3g/calculation-cache.toml",
        "../molecule.xyz",
        "sto-3g",
        HfMethod::Auto,
        0,
        1,
        100,
        1e-8,
        None,
        false,
        true,
    ),
    (
        "h2/sto-3g/calculation.toml",
        "../molecule.xyz",
        "sto-3g",
        HfMethod::Auto,
        0,
        1,
        100,
        1e-8,
        None,
        false,
        false,
    ),
    (
        "h2/sto-3g/mp2_calculation.toml",
        "../molecule.xyz",
        "sto-3g",
        HfMethod::Rhf,
        0,
        1,
        100,
        1e-8,
        None,
        true,
        false,
    ),
    (
        "h2/sto-3g/uhf_h2_plus_calculation.toml",
        "../molecule.xyz",
        "sto-3g",
        HfMethod::Uhf,
        1,
        2,
        100,
        1e-8,
        None,
        false,
        false,
    ),
    (
        "h2o/6-31g/calculation.toml",
        "../h2o.xyz",
        "6-31g",
        HfMethod::Auto,
        0,
        1,
        100,
        1e-8,
        Some(6),
        false,
        false,
    ),
    (
        "h2o/6-31g/mp2_calculation.toml",
        "../h2o.xyz",
        "6-31g",
        HfMethod::Rhf,
        0,
        1,
        80,
        1e-12,
        Some(6),
        true,
        false,
    ),
    (
        "h2o/cc-pvdz/calculation.toml",
        "../h2o.xyz",
        "cc-pvdz",
        HfMethod::Auto,
        0,
        1,
        100,
        1e-8,
        Some(8),
        false,
        false,
    ),
    (
        "h2o/sto-3g/calculation.toml",
        "../h2o.xyz",
        "sto-3g",
        HfMethod::Auto,
        0,
        1,
        100,
        1e-8,
        None,
        false,
        false,
    ),
    (
        "oh/sto-3g/calculation.toml",
        "../oh.xyz",
        "sto-3g",
        HfMethod::Auto,
        0,
        2,
        100,
        1e-5,
        Some(6),
        false,
        false,
    ),
    (
        "oh/sto-3g/mp2_calculation.toml",
        "../oh.xyz",
        "sto-3g",
        HfMethod::Auto,
        0,
        2,
        100,
        1e-5,
        Some(6),
        true,
        false,
    ),
];

#[rstest::rstest]
#[case::anthracene(0)]
#[case::benzene(1)]
#[case::cholesterol(2)]
#[case::ethanol_6_31g(3)]
#[case::ethanol_cc_pvdz(4)]
#[case::ethanol_sto_3g(5)]
#[case::h2_6_31g(6)]
#[case::h2_cc_pvdz(7)]
#[case::h2_cc_pvdz_mp2(8)]
#[case::h2_cache(9)]
#[case::h2_sto_3g(10)]
#[case::h2_sto_3g_mp2(11)]
#[case::h2_plus(12)]
#[case::h2o_6_31g(13)]
#[case::h2o_6_31g_mp2(14)]
#[case::h2o_cc_pvdz(15)]
#[case::h2o_sto_3g(16)]
#[case::oh_sto_3g(17)]
#[case::oh_sto_3g_mp2(18)]
#[allow(
    clippy::float_cmp,
    reason = "The baseline pins literal configuration values exactly"
)]
fn every_valid_sample_resolves_to_expected_scientific_configuration(#[case] case: usize) {
    let (
        file,
        geometry,
        basis,
        method,
        charge,
        multiplicity,
        iterations,
        convergence,
        diis,
        mp2,
        cache,
    ) = SAMPLE_EXPECTATIONS[case];
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("samples");
    let source = fs::read_to_string(root.join(file)).unwrap();
    let parsed = parse_runfile(file, &source).unwrap_or_else(|error| panic!("{file}: {error:?}"));

    assert_eq!(parsed.runfile.molecule.geometry, Path::new(geometry));
    assert_eq!(parsed.runfile.basis.name, basis);
    assert_eq!(parsed.molecule_config.units, Units::Angstrom);
    assert_eq!(parsed.molecule_config.charge.value, charge);
    assert_eq!(
        parsed.molecule_config.multiplicity.value.get(),
        multiplicity
    );
    let hf = parsed
        .hf_config
        .as_ref()
        .expect("All current samples request HF");
    assert_eq!(hf.method.value, method);
    assert_eq!(hf.max_iterations.get(), iterations);
    assert_eq!(hf.convergence_threshold.into_inner(), convergence);
    assert_eq!(hf.diis.enabled, diis.is_some());
    assert_eq!(hf.diis.max_history.value.into_inner(), diis.unwrap_or(6));
    assert_eq!(
        hf.orthogonalization
            .linear_dependency_threshold
            .value
            .into_inner(),
        1e-8
    );
    match (method, hf.guess.value) {
        (HfMethod::Uhf, DensityGuessConfig::OneElectron { perturbation: None })
        | (
            HfMethod::Auto | HfMethod::Rhf,
            DensityGuessConfig::CoreHamiltonian { perturbation: None },
        ) => {}
        (_, guess) => panic!("Unexpected density guess: {guess:?}"),
    }
    assert_eq!(parsed.mp2_config.is_some(), mp2);
    if let Some(config) = &parsed.mp2_config {
        assert_eq!(config.frozen_orbitals.value, 0);
        assert!(matches!(config.memory_limit.value, MemoryLimit::Auto));
    }
    assert_eq!(
        parsed
            .integral_config
            .schwarz_threshold
            .value
            .unwrap()
            .into_inner(),
        1e-12
    );
    assert_eq!(parsed.runfile.cache.enabled, cache);
    assert_eq!(parsed.runfile.output.scf, ScfOutput::Normal);
}

#[test]
fn valid_sample_set_matches_baseline_cases() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("samples");
    let mut actual_files = Vec::new();
    collect_toml_files(&root, &mut actual_files, &root);
    actual_files.retain(|path| path != "invalid_diagnostics.toml");
    actual_files.sort();
    let mut covered_files: Vec<_> = SAMPLE_EXPECTATIONS
        .iter()
        .map(|case| case.0.to_owned())
        .collect();
    covered_files.sort();
    assert_eq!(
        actual_files, covered_files,
        "Every valid sample needs explicit semantic expectations"
    );
}

#[test]
fn omitted_hf_is_currently_absent_from_the_parsed_frontend() {
    // Existing behavior differs from #94's final contract: the CLI calculation
    // supplies HF defaults later, while the future resolved DTO always contains HF.
    let parsed = parse_runfile("omitted-hf.toml", "[basis]\nname = \"sto-3g\"\n").unwrap();
    assert!(parsed.hf_config.is_none());
    assert!(parsed.mp2_config.is_none());
    assert_eq!(
        parsed.runfile.molecule.geometry,
        Path::new("./molecule.xyz")
    );
    assert_eq!(parsed.molecule_config.units, Units::Angstrom);
}
