use rustiq_core::{
    basis::BasisFile,
    calculation::{
        CalculationBuilder, CalculationError, CalculationEvent, CalculationExecution, HfComponent,
        HfSolution, Orbitals,
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
    assert_orbitals(hf.orbitals(), method);
    hf
}

fn assert_orbitals(component: &HfComponent<Orbitals>, method: HfMethod) {
    let check = |orbitals: &Orbitals| {
        assert_eq!(orbitals.coefficients.shape(), (2, 2));
        assert_eq!(orbitals.energies.len(), 2);
        assert_eq!(orbitals.occupied, 1);
        assert!(orbitals.coefficients.iter().all(|x| x.is_finite()));
        assert!(orbitals.energies[0] <= orbitals.energies[1]);
    };
    match (method, component) {
        (HfMethod::Rhf, HfComponent::Rhf(orbitals)) => {
            assert!(component.is_rhf());
            assert!(!component.is_uhf());
            check(orbitals)
        }
        (HfMethod::Uhf, HfComponent::Uhf(spin)) => {
            assert!(!component.is_rhf());
            assert!(component.is_uhf());
            check(&spin.alpha);
            check(&spin.beta);
        }
        _ => panic!("orbital components do not match the requested HF method"),
    }
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
