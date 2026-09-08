//! These tests also run with no default features: no runfile parser or runtime.
use std::num::{NonZeroU8, NonZeroUsize};

use approx::assert_abs_diff_eq;
use miette::{Diagnostic, SourceSpan};
use nalgebra::Point3;
use rustiq_core::{
    basis::{gaussian::basis::Basis, BasisFile},
    calculation::{CalculationError, HfCalculation},
    config::{
        random_config::{
            distribution_config::UniformDistributionConfig, DistributionConfig, RandomConfig,
        },
        validated::{NonNegativeFiniteF64, PositiveFiniteF64},
        DensityGuessConfig, HfConfig, HfMethod, Located, MoleculeConfig, Mp2Config,
        RandomGuessConfig, ResolvedHfMethod,
    },
    hf::{numerical_error::NumericalError, scf::ScfSetupError},
    molecules::{atom::Atom, geometry::Geometry, molecule::Molecule, units::Units},
    mp2::Mp2Error,
};

fn geometry() -> Geometry {
    let hydrogen = &periodic_table::periodic_table()[0];
    Geometry::new(
        "H2".into(),
        vec![
            Atom::new(hydrogen, Point3::new(0.0, 0.0, -0.37)),
            Atom::new(hydrogen, Point3::new(0.0, 0.0, 0.37)),
        ],
    )
}

fn input() -> (Molecule, Basis) {
    let mut molecule = MoleculeConfig {
        units: Units::Angstrom,
        ..Default::default()
    }
    .build(geometry())
    .unwrap();
    molecule.convert_to(Units::Bohr);
    let file = BasisFile::from_reader(&include_bytes!("data/sto-3g.json")[..]).unwrap();
    let basis = Basis::try_load(&file, &molecule).unwrap();
    (molecule, basis)
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
fn public_configuration_runs_rhf_and_uhf_mp2_without_a_frontend() {
    let (molecule, basis) = input();
    for (method, resolved) in [
        (HfMethod::Auto, ResolvedHfMethod::Rhf),
        (HfMethod::Uhf, ResolvedHfMethod::Uhf),
    ] {
        let config = HfConfig {
            method,
            diis: true,
            convergence_threshold: PositiveFiniteF64::try_new(1e-12).unwrap(),
            ..Default::default()
        };
        let mut calculation = HfCalculation::new(&molecule, &basis, &config).unwrap();
        assert_eq!(calculation.method(), resolved);
        assert!(matches!(
            calculation.mp2(&Mp2Config::default()),
            Err(CalculationError::HfNotConverged { iterations: 0 })
        ));
        let result = calculation.run().unwrap();
        assert!(result.converged);
        assert_abs_diff_eq!(
            result.electronic_energy,
            -1.831_863_646_477_507,
            epsilon = 1e-10
        );
        let mp2 = calculation.mp2(&Mp2Config::default()).unwrap();
        assert_abs_diff_eq!(
            mp2.correlation_energy,
            -0.013_138_073_589_533,
            epsilon = 1e-11
        );

        for span in [None, Some((12, 1).into())] {
            let error = calculation
                .mp2(&Mp2Config {
                    frozen_orbitals: 2,
                    frozen_orbitals_span: span,
                })
                .unwrap_err();
            assert!(matches!(
                error,
                CalculationError::Mp2 {
                    error: Mp2Error::InvalidFrozenOrbitalCount { .. },
                    ..
                }
            ));
            assert_eq!(labels(&error), span.into_iter().collect::<Vec<_>>());
            assert!(error.source_code().is_none());
        }
    }
}

#[test]
fn public_api_rejects_mp2_after_unconverged_hf() {
    let (molecule, basis) = input();
    for method in [HfMethod::Rhf, HfMethod::Uhf] {
        let config = HfConfig {
            method,
            max_iterations: NonZeroUsize::MIN,
            ..Default::default()
        };
        let mut calculation = HfCalculation::new(&molecule, &basis, &config).unwrap();
        assert!(!calculation.run().unwrap().converged);
        assert!(matches!(
            calculation.mp2(&Mp2Config::default()),
            Err(CalculationError::HfNotConverged { iterations: 1 })
        ));
    }
}

#[test]
fn scientific_method_errors_preserve_optional_frontend_spans() {
    let molecule = MoleculeConfig {
        charge: 1.into(),
        multiplicity: NonZeroU8::new(2).unwrap().into(),
        ..Default::default()
    }
    .build(geometry())
    .unwrap();
    assert_eq!(
        HfConfig::default().resolve_method(&molecule).unwrap(),
        ResolvedHfMethod::Uhf
    );
    for span in [None, Some((7, 5).into())] {
        let mut config = HfConfig {
            method: HfMethod::Rhf,
            ..Default::default()
        };
        config.source_spans.method = span;
        let error = config.resolve_method(&molecule).unwrap_err();
        assert!(matches!(error, CalculationError::Method { .. }));
        assert_eq!(labels(&error), span.into_iter().collect::<Vec<_>>());
        assert!(error.source_code().is_none());
    }
}

#[test]
fn molecular_state_errors_can_label_both_related_values() {
    let config = MoleculeConfig {
        charge: Located {
            value: 1,
            span: Some((3, 1).into()),
        },
        multiplicity: Located {
            value: NonZeroU8::MIN,
            span: Some((20, 1).into()),
        },
        ..Default::default()
    };
    let error = config.build(geometry()).err().unwrap();
    assert!(matches!(error, CalculationError::Molecule { .. }));
    assert_eq!(labels(&error), vec![(3, 1).into(), (20, 1).into()]);
    assert!(error.source_code().is_none());
}

#[test]
fn setup_errors_retain_threshold_and_guess_locations() {
    let (molecule, basis) = input();
    let span: SourceSpan = (30, 4).into();
    for method in [HfMethod::Rhf, HfMethod::Uhf] {
        let mut config = HfConfig {
            method,
            linear_dependency_threshold: NonNegativeFiniteF64::try_new(1.0).unwrap(),
            ..Default::default()
        };
        config.source_spans.linear_dependency_threshold = Some(span);
        let error = HfCalculation::new(&molecule, &basis, &config)
            .err()
            .unwrap();
        assert_eq!(labels(&error), vec![span]);
        if method == HfMethod::Rhf {
            assert!(matches!(
                error,
                CalculationError::RhfSetup {
                    error: ScfSetupError::Numerical(NumericalError::InsufficientOverlapRank { .. }),
                    ..
                }
            ));
        }
        config.linear_dependency_threshold = HfConfig::default().linear_dependency_threshold;
        config.guess = DensityGuessConfig::Random {
            config: RandomGuessConfig {
                random: RandomConfig {
                    seed: Some(42),
                    distribution: DistributionConfig::Uniform {
                        config: UniformDistributionConfig {
                            min: 1.0,
                            max: -1.0,
                        },
                    },
                },
            },
        };
        config.source_spans.guess = Some(span);
        let error = HfCalculation::new(&molecule, &basis, &config)
            .err()
            .unwrap();
        assert_eq!(labels(&error), vec![span]);
    }
}

#[cfg(feature = "runfile")]
#[test]
fn toml_adapter_preserves_original_spans_and_numerical_behavior() {
    use rustiq_core::runfile::parser::parse_runfile;
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

    let (molecule, basis) = input();
    let error = HfCalculation::new(&molecule, &basis, &config)
        .err()
        .unwrap();
    let span = labels(&error)[0];
    assert_eq!(&source[span.offset()..span.offset() + span.len()], "1.0");
    let mut calculation = HfCalculation::new(&molecule, &basis, &HfConfig::default()).unwrap();
    calculation.run().unwrap();
    let error = calculation.mp2(&parsed.mp2_config.unwrap()).unwrap_err();
    let span = labels(&error)[0];
    assert_eq!(span.offset(), source.rfind('1').unwrap());
    assert_eq!(span.len(), 1);

    let parsed = parse_runfile("defaults.toml", "[global]\nbasis = 'sto-3g'\n[hf]\n").unwrap();
    let from_toml = parsed.hf_config.unwrap();
    assert!(from_toml.source_spans.method.is_none());
    assert!(from_toml.source_spans.guess.is_none());
    assert!(from_toml.source_spans.linear_dependency_threshold.is_none());
    let mut adapted = HfCalculation::new(&molecule, &basis, &from_toml).unwrap();
    let adapted_result = adapted.run().unwrap();
    let direct_result = calculation.run().unwrap();
    assert_abs_diff_eq!(
        adapted_result.total_energy,
        direct_result.total_energy,
        epsilon = 1e-10
    );
}
