use rustiq_core::{
    basis::BasisFile,
    calculation::{
        CalculationBuilder, CalculationError, CalculationEvent, CalculationExecution, HfSolution,
    },
    config::{HfConfig, HfMethod, MoleculeConfig, Mp2Config},
    molecules::{geometry::Geometry, units::Units},
};

fn solution(method: HfMethod, iterations: usize) -> HfSolution {
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
fn hf_outlives_inputs_and_can_retry_mp2_after_error() {
    for method in [HfMethod::Rhf, HfMethod::Uhf] {
        let hf = solution(method, 100);
        let first = hf.mp2(Mp2Config::default()).unwrap();
        let error = hf
            .mp2(Mp2Config {
                frozen_orbitals: 99.into(),
            })
            .unwrap_err();
        assert!(matches!(error.cause(), CalculationError::Mp2 { .. }));
        assert!(error.hf().unwrap().summary().scf.converged);
        drop(hf);
        let recovered = error.into_hf().unwrap();
        assert_eq!(first, recovered.mp2(Mp2Config::default()).unwrap());
        assert_eq!(first, recovered.clone().mp2(Mp2Config::default()).unwrap());
    }
}

#[test]
fn unconverged_hf_is_retained_but_cannot_run_mp2() {
    for method in [HfMethod::Rhf, HfMethod::Uhf] {
        let hf = solution(method, 1);
        assert!(!hf.summary().scf.converged);
        let error = hf.mp2(Mp2Config::default()).unwrap_err();
        assert!(matches!(
            error.cause(),
            CalculationError::HfNotConverged { iterations: 1 }
        ));
        assert!(!error.into_hf().unwrap().summary().scf.converged);
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
