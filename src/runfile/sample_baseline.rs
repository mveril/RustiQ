//! Migration baseline for the production frontend, independent of rendering and source spans.
// Component tests in parser, hf, mp2, random_config, output, and diagnostics
// already protect spans, validation, distributions, memory syntax, and round trips.
// This baseline adds the missing sample-wide assertions on production core conversion.
#![allow(
    clippy::float_cmp,
    reason = "The baseline pins literal configuration values exactly"
)]
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
                    .replace('\\', "/"),
            );
        }
    }
}

#[derive(Clone, Copy)]
struct SampleExpectation {
    path: &'static str,
    geometry: &'static str,
    basis: &'static str,
    hf_method: HfMethod,
    charge: i32,
    multiplicity: u8,
    max_iterations: usize,
    convergence_threshold: f64,
    diis: DiisExpectation,
    guess: GuessExpectation,
    mp2: Option<Mp2Expectation>,
    cache_enabled: bool,
}

#[derive(Clone, Copy)]
struct DiisExpectation {
    enabled: bool,
    max_history: usize,
}

#[derive(Clone, Copy)]
enum GuessExpectation {
    CoreHamiltonian,
    OneElectron,
}

#[derive(Clone, Copy)]
struct Mp2Expectation {
    frozen_orbitals: usize,
    memory_limit: MemoryLimitExpectation,
}

#[derive(Clone, Copy)]
enum MemoryLimitExpectation {
    Auto,
}

const SAMPLE_EXPECTATIONS: &[SampleExpectation] = &[
    SampleExpectation {
        path: "anthracene/cc-pvdz/calculation.toml",
        geometry: "../anthracene.xyz",
        basis: "cc-pvdz",
        hf_method: HfMethod::Auto,
        charge: 0,
        multiplicity: 1,
        max_iterations: 300,
        convergence_threshold: 1e-8,
        diis: DiisExpectation {
            enabled: true,
            max_history: 6,
        },
        guess: GuessExpectation::CoreHamiltonian,
        mp2: Some(Mp2Expectation {
            frozen_orbitals: 0,
            memory_limit: MemoryLimitExpectation::Auto,
        }),
        cache_enabled: true,
    },
    SampleExpectation {
        path: "benzene/cc-pvdz/calculation.toml",
        geometry: "../benzene.xyz",
        basis: "cc-pvdz",
        hf_method: HfMethod::Auto,
        charge: 0,
        multiplicity: 1,
        max_iterations: 150,
        convergence_threshold: 1e-10,
        diis: DiisExpectation {
            enabled: true,
            max_history: 10,
        },
        guess: GuessExpectation::CoreHamiltonian,
        mp2: None,
        cache_enabled: false,
    },
    SampleExpectation {
        path: "cholesterol/sto-3g/calculation.toml",
        geometry: "../cholesterol.xyz",
        basis: "sto-3g",
        hf_method: HfMethod::Auto,
        charge: 0,
        multiplicity: 1,
        max_iterations: 100,
        convergence_threshold: 1e-8,
        diis: DiisExpectation {
            enabled: true,
            max_history: 8,
        },
        guess: GuessExpectation::CoreHamiltonian,
        mp2: Some(Mp2Expectation {
            frozen_orbitals: 0,
            memory_limit: MemoryLimitExpectation::Auto,
        }),
        cache_enabled: false,
    },
    SampleExpectation {
        path: "ethanol/6-31g/calculation.toml",
        geometry: "../ethanol.xyz",
        basis: "6-31g",
        hf_method: HfMethod::Auto,
        charge: 0,
        multiplicity: 1,
        max_iterations: 100,
        convergence_threshold: 1e-10,
        diis: DiisExpectation {
            enabled: true,
            max_history: 8,
        },
        guess: GuessExpectation::CoreHamiltonian,
        mp2: None,
        cache_enabled: false,
    },
    SampleExpectation {
        path: "ethanol/cc-pvdz/calculation.toml",
        geometry: "../ethanol.xyz",
        basis: "cc-pvdz",
        hf_method: HfMethod::Auto,
        charge: 0,
        multiplicity: 1,
        max_iterations: 120,
        convergence_threshold: 1e-10,
        diis: DiisExpectation {
            enabled: true,
            max_history: 10,
        },
        guess: GuessExpectation::CoreHamiltonian,
        mp2: None,
        cache_enabled: false,
    },
    SampleExpectation {
        path: "ethanol/sto-3g/calculation.toml",
        geometry: "../ethanol.xyz",
        basis: "sto-3g",
        hf_method: HfMethod::Auto,
        charge: 0,
        multiplicity: 1,
        max_iterations: 80,
        convergence_threshold: 1e-10,
        diis: DiisExpectation {
            enabled: true,
            max_history: 8,
        },
        guess: GuessExpectation::CoreHamiltonian,
        mp2: None,
        cache_enabled: false,
    },
    SampleExpectation {
        path: "h2/6-31g/calculation.toml",
        geometry: "../molecule.xyz",
        basis: "6-31g",
        hf_method: HfMethod::Auto,
        charge: 0,
        multiplicity: 1,
        max_iterations: 100,
        convergence_threshold: 1e-8,
        diis: DiisExpectation {
            enabled: true,
            max_history: 2,
        },
        guess: GuessExpectation::CoreHamiltonian,
        mp2: None,
        cache_enabled: false,
    },
    SampleExpectation {
        path: "h2/cc-pvdz/calculation.toml",
        geometry: "../molecule.xyz",
        basis: "cc-pvdz",
        hf_method: HfMethod::Auto,
        charge: 0,
        multiplicity: 1,
        max_iterations: 100,
        convergence_threshold: 1e-8,
        diis: DiisExpectation {
            enabled: true,
            max_history: 8,
        },
        guess: GuessExpectation::CoreHamiltonian,
        mp2: None,
        cache_enabled: false,
    },
    SampleExpectation {
        path: "h2/cc-pvdz/mp2_calculation.toml",
        geometry: "../molecule.xyz",
        basis: "cc-pvdz",
        hf_method: HfMethod::Rhf,
        charge: 0,
        multiplicity: 1,
        max_iterations: 80,
        convergence_threshold: 1e-12,
        diis: DiisExpectation {
            enabled: true,
            max_history: 8,
        },
        guess: GuessExpectation::CoreHamiltonian,
        mp2: Some(Mp2Expectation {
            frozen_orbitals: 0,
            memory_limit: MemoryLimitExpectation::Auto,
        }),
        cache_enabled: false,
    },
    SampleExpectation {
        path: "h2/sto-3g/calculation-cache.toml",
        geometry: "../molecule.xyz",
        basis: "sto-3g",
        hf_method: HfMethod::Auto,
        charge: 0,
        multiplicity: 1,
        max_iterations: 100,
        convergence_threshold: 1e-8,
        diis: DiisExpectation {
            enabled: false,
            max_history: 6,
        },
        guess: GuessExpectation::CoreHamiltonian,
        mp2: None,
        cache_enabled: true,
    },
    SampleExpectation {
        path: "h2/sto-3g/calculation.toml",
        geometry: "../molecule.xyz",
        basis: "sto-3g",
        hf_method: HfMethod::Auto,
        charge: 0,
        multiplicity: 1,
        max_iterations: 100,
        convergence_threshold: 1e-8,
        diis: DiisExpectation {
            enabled: false,
            max_history: 6,
        },
        guess: GuessExpectation::CoreHamiltonian,
        mp2: None,
        cache_enabled: false,
    },
    SampleExpectation {
        path: "h2/sto-3g/mp2_calculation.toml",
        geometry: "../molecule.xyz",
        basis: "sto-3g",
        hf_method: HfMethod::Rhf,
        charge: 0,
        multiplicity: 1,
        max_iterations: 100,
        convergence_threshold: 1e-8,
        diis: DiisExpectation {
            enabled: false,
            max_history: 6,
        },
        guess: GuessExpectation::CoreHamiltonian,
        mp2: Some(Mp2Expectation {
            frozen_orbitals: 0,
            memory_limit: MemoryLimitExpectation::Auto,
        }),
        cache_enabled: false,
    },
    SampleExpectation {
        path: "h2/sto-3g/uhf_h2_plus_calculation.toml",
        geometry: "../molecule.xyz",
        basis: "sto-3g",
        hf_method: HfMethod::Uhf,
        charge: 1,
        multiplicity: 2,
        max_iterations: 100,
        convergence_threshold: 1e-8,
        diis: DiisExpectation {
            enabled: false,
            max_history: 6,
        },
        guess: GuessExpectation::OneElectron,
        mp2: None,
        cache_enabled: false,
    },
    SampleExpectation {
        path: "h2o/6-31g/calculation.toml",
        geometry: "../h2o.xyz",
        basis: "6-31g",
        hf_method: HfMethod::Auto,
        charge: 0,
        multiplicity: 1,
        max_iterations: 100,
        convergence_threshold: 1e-8,
        diis: DiisExpectation {
            enabled: true,
            max_history: 6,
        },
        guess: GuessExpectation::CoreHamiltonian,
        mp2: None,
        cache_enabled: false,
    },
    SampleExpectation {
        path: "h2o/6-31g/mp2_calculation.toml",
        geometry: "../h2o.xyz",
        basis: "6-31g",
        hf_method: HfMethod::Rhf,
        charge: 0,
        multiplicity: 1,
        max_iterations: 80,
        convergence_threshold: 1e-12,
        diis: DiisExpectation {
            enabled: true,
            max_history: 6,
        },
        guess: GuessExpectation::CoreHamiltonian,
        mp2: Some(Mp2Expectation {
            frozen_orbitals: 0,
            memory_limit: MemoryLimitExpectation::Auto,
        }),
        cache_enabled: false,
    },
    SampleExpectation {
        path: "h2o/cc-pvdz/calculation.toml",
        geometry: "../h2o.xyz",
        basis: "cc-pvdz",
        hf_method: HfMethod::Auto,
        charge: 0,
        multiplicity: 1,
        max_iterations: 100,
        convergence_threshold: 1e-8,
        diis: DiisExpectation {
            enabled: true,
            max_history: 8,
        },
        guess: GuessExpectation::CoreHamiltonian,
        mp2: None,
        cache_enabled: false,
    },
    SampleExpectation {
        path: "h2o/sto-3g/calculation.toml",
        geometry: "../h2o.xyz",
        basis: "sto-3g",
        hf_method: HfMethod::Auto,
        charge: 0,
        multiplicity: 1,
        max_iterations: 100,
        convergence_threshold: 1e-8,
        diis: DiisExpectation {
            enabled: false,
            max_history: 6,
        },
        guess: GuessExpectation::CoreHamiltonian,
        mp2: None,
        cache_enabled: false,
    },
    SampleExpectation {
        path: "oh/sto-3g/calculation.toml",
        geometry: "../oh.xyz",
        basis: "sto-3g",
        hf_method: HfMethod::Auto,
        charge: 0,
        multiplicity: 2,
        max_iterations: 100,
        convergence_threshold: 1e-5,
        diis: DiisExpectation {
            enabled: true,
            max_history: 6,
        },
        guess: GuessExpectation::CoreHamiltonian,
        mp2: None,
        cache_enabled: false,
    },
    SampleExpectation {
        path: "oh/sto-3g/mp2_calculation.toml",
        geometry: "../oh.xyz",
        basis: "sto-3g",
        hf_method: HfMethod::Auto,
        charge: 0,
        multiplicity: 2,
        max_iterations: 100,
        convergence_threshold: 1e-10,
        diis: DiisExpectation {
            enabled: true,
            max_history: 6,
        },
        guess: GuessExpectation::CoreHamiltonian,
        mp2: Some(Mp2Expectation {
            frozen_orbitals: 0,
            memory_limit: MemoryLimitExpectation::Auto,
        }),
        cache_enabled: false,
    },
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
fn every_valid_sample_has_expected_scientific_configuration(#[case] case: usize) {
    let expected = SAMPLE_EXPECTATIONS[case];
    let file = expected.path;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("samples");
    let source = fs::read_to_string(root.join(file)).unwrap();
    let parsed = parse_runfile(file, &source).unwrap_or_else(|error| panic!("{file}: {error:?}"));

    assert_eq!(
        parsed.runfile.molecule.geometry,
        Path::new(expected.geometry)
    );
    assert_eq!(parsed.runfile.basis.name, expected.basis);
    assert_eq!(parsed.molecule_config.units, Units::Angstrom);
    assert_eq!(parsed.molecule_config.charge.value, expected.charge);
    assert_eq!(
        parsed.molecule_config.multiplicity.value.get(),
        expected.multiplicity
    );
    let hf = parsed
        .hf_config
        .as_ref()
        .expect("All current samples request HF");
    assert_eq!(hf.method.value, expected.hf_method);
    assert_eq!(hf.max_iterations.get(), expected.max_iterations);
    assert_eq!(
        hf.convergence_threshold.into_inner(),
        expected.convergence_threshold
    );
    assert_eq!(hf.diis.enabled, expected.diis.enabled);
    assert_eq!(
        hf.diis.max_history.value.into_inner(),
        expected.diis.max_history
    );
    assert_eq!(
        hf.orthogonalization
            .linear_dependency_threshold
            .value
            .into_inner(),
        1e-8
    );
    match (expected.guess, hf.guess.value) {
        (GuessExpectation::OneElectron, DensityGuessConfig::OneElectron { perturbation: None })
        | (
            GuessExpectation::CoreHamiltonian,
            DensityGuessConfig::CoreHamiltonian { perturbation: None },
        ) => {}
        (_, guess) => panic!("Unexpected density guess: {guess:?}"),
    }
    match (expected.mp2, &parsed.mp2_config) {
        (None, None) => {}
        (Some(expected), Some(config)) => {
            assert_eq!(config.frozen_orbitals.value, expected.frozen_orbitals);
            match (expected.memory_limit, config.memory_limit.value) {
                (MemoryLimitExpectation::Auto, MemoryLimit::Auto) => {}
                _ => panic!(
                    "Unexpected MP2 memory limit: {:?}",
                    config.memory_limit.value
                ),
            }
        }
        _ => panic!("MP2 presence differs from the expected configuration"),
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
    assert_eq!(parsed.runfile.cache.enabled, expected.cache_enabled);
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
        .map(|case| case.path.to_owned())
        .collect();
    covered_files.sort();
    assert_eq!(
        actual_files, covered_files,
        "Every valid sample needs explicit semantic expectations"
    );
}

#[test]
fn omitted_hf_is_resolved_before_core_conversion() {
    // The legacy syntax retains omission; the production resolved boundary materializes HF.
    let parsed = parse_runfile("omitted-hf.toml", "[basis]\nname = \"sto-3g\"\n").unwrap();
    assert!(parsed.runfile.method.hf.is_none());
    let hf = parsed.hf_config.unwrap();
    assert_eq!(hf.method.value, HfMethod::Auto);
    assert_eq!(hf.max_iterations.get(), 100);
    assert!(hf.method.span.is_none());
    assert!(parsed.mp2_config.is_none());
    assert_eq!(
        parsed.runfile.molecule.geometry,
        Path::new("./molecule.xyz")
    );
    assert_eq!(parsed.molecule_config.units, Units::Angstrom);
}
