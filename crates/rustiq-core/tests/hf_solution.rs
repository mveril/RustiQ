use rustiq_core::{
    basis::BasisFile,
    calculation::{
        CalculationBuilder, CalculationError, CalculationEvent, CalculationExecution, HfOutcome,
    },
    config::{HfConfig, HfMethod, MoleculeConfig, Mp2Config, ResolvedHfMethod},
    molecules::{geometry::Geometry, units::Units},
};

fn solution(method: HfMethod, iterations: usize) -> HfOutcome {
    let geometry =
        Geometry::from_reader(&include_bytes!("data/samples/h2/molecule.xyz")[..]).unwrap();
    let file = BasisFile::from_reader(&include_bytes!("data/sto-3g.json")[..]).unwrap();
    let prepared = CalculationBuilder::new(&geometry, &file)
        .with_molecule_config(MoleculeConfig {
            units: Units::Angstrom,
            ..Default::default()
        })
        .with_hf(HfConfig {
            method: method.into(),
            max_iterations: iterations.try_into().unwrap(),
            ..Default::default()
        })
        // run_hf must ignore MP2 configuration, including invalid configurations.
        .with_mp2(Mp2Config {
            frozen_orbitals: 99.into(),
        })
        .prepare()
        .unwrap();
    let mut mp2_events = 0;
    let hf = prepared
        .run_hf_with_events(|event| {
            if matches!(event, CalculationEvent::Mp2Completed { .. }) {
                mp2_events += 1;
            }
        })
        .unwrap();
    assert_eq!(mp2_events, 0);
    hf
}

#[test]
fn outcome_reports_resolved_method_for_both_convergence_states() {
    for (method, expected) in [
        (HfMethod::Auto, ResolvedHfMethod::Rhf),
        (HfMethod::Rhf, ResolvedHfMethod::Rhf),
        (HfMethod::Uhf, ResolvedHfMethod::Uhf),
    ] {
        for iterations in [1, 100] {
            let outcome = solution(method, iterations);
            assert_eq!(outcome.method(), expected);
            assert_eq!(outcome.is_converged(), iterations == 100);
            match outcome {
                HfOutcome::Converged(hf) => assert_eq!(hf.method(), expected),
                HfOutcome::Unconverged(hf) => assert_eq!(hf.method(), expected),
            }
        }
    }
}

#[test]
fn hf_outlives_inputs_and_can_retry_mp2_after_error() {
    for (method, resolved_method) in [
        (HfMethod::Rhf, ResolvedHfMethod::Rhf),
        (HfMethod::Uhf, ResolvedHfMethod::Uhf),
    ] {
        let HfOutcome::Converged(hf) = solution(method, 100) else {
            panic!("expected convergence");
        };
        assert_eq!(hf.method(), resolved_method);
        let first = hf.mp2(Mp2Config::default()).unwrap();
        assert!(std::ptr::eq(hf.summary(), hf.clone().summary()));
        let error = hf
            .mp2(Mp2Config {
                frozen_orbitals: 99.into(),
            })
            .unwrap_err();
        assert!(matches!(error.cause(), CalculationError::Mp2 { .. }));
        assert!(matches!(error.hf(), Some(HfOutcome::Converged(_))));
        drop(hf);
        let HfOutcome::Converged(recovered) = error.into_hf().unwrap() else {
            panic!("expected convergence");
        };
        assert_eq!(first, recovered.mp2(Mp2Config::default()).unwrap());
        assert_eq!(first, recovered.clone().mp2(Mp2Config::default()).unwrap());
    }
}

#[test]
fn unconverged_hf_is_retained_but_cannot_run_mp2() {
    for method in [HfMethod::Rhf, HfMethod::Uhf] {
        let hf = solution(method, 1);
        let HfOutcome::Unconverged(hf) = hf else {
            panic!("expected unconverged HF");
        };
        assert_eq!(hf.summary().scf.iterations, 1);
        assert!(hf.summary().scf.total_energy.is_finite());
        assert_eq!(
            hf.clone().summary().scf.total_energy,
            hf.summary().scf.total_energy
        );
    }
}

#[test]
fn preparation_error_has_no_hf() {
    let geometry =
        Geometry::from_reader(&include_bytes!("data/samples/h2/molecule.xyz")[..]).unwrap();
    let file = BasisFile::from_reader(&include_bytes!("data/sto-3g.json")[..]).unwrap();
    let error = CalculationBuilder::new(&geometry, &file)
        .with_molecule_config(MoleculeConfig {
            charge: 99.into(),
            ..Default::default()
        })
        .execute()
        .unwrap_err();
    assert!(error.hf().is_none());
    assert!(matches!(error.cause(), CalculationError::Molecule { .. }));
}
