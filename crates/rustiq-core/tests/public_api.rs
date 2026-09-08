//! These tests also run with no default features: no runfile parser or runtime.
use std::num::{NonZeroU8, NonZeroUsize};

use approx::assert_abs_diff_eq;
use miette::{Diagnostic, SourceSpan};
use nalgebra::Point3;
use rustiq_core::{
    basis::{gaussian::basis::Basis, BasisFile},
    calculation::{
        CalculationBuilder, CalculationError, CalculationExecution, CalculationObserver,
        HfCalculation, HfCalculationResult,
    },
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

#[derive(Default)]
struct WorkflowObserver(Vec<&'static str>);

impl rustiq_core::hf::scf_observer::ScfObserver for WorkflowObserver {
    fn on_iteration(&mut self, _: &rustiq_core::hf::scf_iteration::ScfIteration) {
        self.0.push("iteration");
    }
}

impl CalculationObserver for WorkflowObserver {
    fn on_basis_start(&mut self) {
        self.0.push("basis_start");
    }
    fn on_basis_ready(&mut self, _: &Basis, _: std::time::Duration) {
        self.0.push("basis_ready");
    }
    fn on_hf_start(&mut self, _: ResolvedHfMethod, _: &HfConfig) {
        self.0.push("hf_start");
    }
    fn on_hf_complete(&mut self, _: &HfCalculationResult) {
        self.0.push("hf_complete");
    }
    fn on_mp2_complete(&mut self, _: &HfCalculationResult, _: &rustiq_core::mp2::Mp2Result) {
        self.0.push("mp2_complete");
    }
}

#[test]
fn calculation_builder_normalizes_units_and_orchestrates_both_hf_methods_and_mp2() {
    let geometry = geometry();
    let file = BasisFile::from_reader(&include_bytes!("data/sto-3g.json")[..]).unwrap();
    for method in [HfMethod::Rhf, HfMethod::Uhf] {
        let builder = CalculationBuilder::new(&geometry, &file)
            .with_molecule_config(MoleculeConfig {
                units: Units::Angstrom,
                ..Default::default()
            })
            .with_hf(HfConfig {
                method: method.into(),
                ..Default::default()
            })
            .with_mp2(Mp2Config::default());
        let mut observer = WorkflowObserver::default();
        let result = builder.execute_with_observer(&mut observer).unwrap();
        assert_abs_diff_eq!(
            result.hf.as_ref().unwrap().scf.total_energy,
            -1.116_759_307_506_361_3,
            epsilon = 1e-10
        );
        assert_abs_diff_eq!(
            result.mp2.unwrap().correlation_energy,
            -0.013_138_073_589_533,
            epsilon = 1e-10
        );
        assert_eq!(
            &observer.0[..3],
            &["basis_start", "basis_ready", "hf_start"]
        );
        assert!(observer.0.contains(&"iteration"));
        assert_eq!(
            &observer.0[observer.0.len() - 2..],
            &["hf_complete", "mp2_complete"]
        );

        let prepared = builder.prepare().unwrap();
        assert_eq!(prepared.get_molecule().unit(), Units::Bohr);
        assert_eq!(prepared.get_basis().nbasis(), 2);
        // Preparation leaves the caller's Angstrom geometry intact and can be reused.
        assert_eq!(geometry.atoms[0].position.z, -0.37);
        let repeated = prepared.execute().unwrap();
        assert_abs_diff_eq!(
            repeated.hf.unwrap().scf.total_energy,
            result.hf.unwrap().scf.total_energy,
            epsilon = 1e-10
        );
    }
}

#[test]
fn builder_mutable_setters_enforce_mp2_dependency_and_support_preparation_only() {
    let geometry = geometry();
    let file = BasisFile::from_reader(&include_bytes!("data/sto-3g.json")[..]).unwrap();
    let mut builder = CalculationBuilder::new(&geometry, &file);
    builder.hf(None).mp2(Mp2Config::default());
    let mut observer = WorkflowObserver::default();
    assert!(matches!(
        builder.execute_with_observer(&mut observer),
        Err(CalculationError::Mp2RequiresHf)
    ));
    assert!(observer.0.is_empty());
    builder.mp2(None).molecule_config(MoleculeConfig {
        units: Units::Angstrom,
        ..Default::default()
    });
    assert!(builder.get_hf().is_none());
    assert!(builder.get_mp2().is_none());
    let result = builder.execute_with_observer(&mut observer).unwrap();
    assert!(result.hf.is_none());
    assert!(result.mp2.is_none());
    assert_eq!(observer.0, vec!["basis_start", "basis_ready"]);
}

#[test]
fn builder_never_runs_mp2_after_unconverged_hf() {
    let geometry = geometry();
    let file = BasisFile::from_reader(&include_bytes!("data/sto-3g.json")[..]).unwrap();
    let builder = CalculationBuilder::new(&geometry, &file)
        .with_hf(HfConfig {
            max_iterations: NonZeroUsize::MIN,
            ..Default::default()
        })
        .with_mp2(Mp2Config::default());
    let mut observer = WorkflowObserver::default();
    assert!(matches!(
        builder.execute_with_observer(&mut observer),
        Err(CalculationError::HfNotConverged { iterations: 1 })
    ));
    assert_eq!(observer.0.last(), Some(&"hf_complete"));
    assert!(!observer.0.contains(&"mp2_complete"));
}

#[test]
fn builder_retains_typed_method_and_basis_errors() {
    let geometry = geometry();
    let file = BasisFile::from_reader(&include_bytes!("data/sto-3g.json")[..]).unwrap();
    let builder = CalculationBuilder::new(&geometry, &file)
        .with_molecule_config(MoleculeConfig {
            charge: 1.into(),
            multiplicity: NonZeroU8::new(2).unwrap().into(),
            ..Default::default()
        })
        .with_hf(HfConfig {
            method: Located {
                value: HfMethod::Rhf,
                span: Some((8, 5).into()),
            },
            ..Default::default()
        });
    let error = builder.execute().unwrap_err();
    assert!(matches!(error, CalculationError::Method { .. }));
    assert_eq!(labels(&error), vec![(8, 5).into()]);

    let mut data: serde_json::Value =
        serde_json::from_slice(include_bytes!("data/sto-3g.json")).unwrap();
    data["elements"]["1"]["electron_shells"][0]["exponents"][0] = "0.0".into();
    let bytes = serde_json::to_vec(&data).unwrap();
    let invalid = BasisFile::from_reader(&bytes[..]).unwrap();
    assert!(matches!(
        CalculationBuilder::new(&geometry, &invalid).execute(),
        Err(CalculationError::Basis(_))
    ));
}

#[test]
fn located_values_support_owned_extraction_with_or_without_provenance() {
    let plain = Located::from(String::from("direct Rust input"));
    assert!(plain.span.is_none());
    assert_eq!(plain.into_inner(), "direct Rust input");
    let sourced = Located {
        value: String::from("frontend input"),
        span: Some((4, 14).into()),
    };
    assert_eq!(sourced.into_inner(), "frontend input");
    let defaults = Mp2Config::default();
    assert_eq!(defaults.frozen_orbitals.into_inner(), 0);
    assert!(defaults.frozen_orbitals.span.is_none());
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
            method: method.into(),
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
                    frozen_orbitals: Located { value: 2, span },
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
            method: method.into(),
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
            method: HfMethod::Rhf.into(),
            ..Default::default()
        };
        config.method.span = span;
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
            method: method.into(),
            linear_dependency_threshold: NonNegativeFiniteF64::try_new(1.0).unwrap().into(),
            ..Default::default()
        };
        config.linear_dependency_threshold.span = Some(span);
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
        config.guess.value = DensityGuessConfig::Random {
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
        config.guess.span = Some(span);
        let error = HfCalculation::new(&molecule, &basis, &config)
            .err()
            .unwrap();
        assert_eq!(labels(&error), vec![span]);
    }
}
