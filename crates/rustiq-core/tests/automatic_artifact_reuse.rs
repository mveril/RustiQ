use rustiq_core::{
    basis::BasisFile,
    calculation::{
        ArtifactReuseDecision, CalculationBuilder, CalculationEvent, CalculationExecution,
    },
    config::Mp2Config,
    molecules::geometry::Geometry,
    persistence::{CompactEri, EriCache, RustiQData},
};

fn inputs(distance: f64) -> (Geometry, BasisFile) {
    let geometry = Geometry::from_source(
        "h2.xyz",
        &format!("2\nH2 in Bohr\nH 0 0 0\nH {distance} 0 0\n"),
    )
    .unwrap();
    let basis = BasisFile::from_reader(&include_bytes!("data/sto-3g.json")[..]).unwrap();
    (geometry, basis)
}

#[test]
fn execute_automatically_reuses_portable_eri_and_preserves_mp2_results() {
    let (geometry, basis) = inputs(1.4);
    let original = CalculationBuilder::new(&geometry, &basis)
        .with_mp2(Mp2Config::default())
        .prepare()
        .unwrap();
    let expected = original.execute().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("h2.rustiq");
    RustiQData::from_calculation(&original)
        .unwrap()
        .write_with_eri(&path, expected.hf.ao_eri())
        .unwrap();
    let archive_bytes = std::fs::read(&path).unwrap();
    let restored = RustiQData::open(&path)
        .unwrap()
        .prepare_calculation()
        .unwrap();
    let mut decisions = Vec::new();
    let result = restored
        .execute_with_events(|event| {
            if let CalculationEvent::ArtifactReuse(event) = event {
                assert_eq!(event.artifact, "ao_eri");
                decisions.push(event.decision);
            }
        })
        .unwrap();
    assert_eq!(decisions, [ArtifactReuseDecision::Reused]);
    assert_eq!(
        result.hf.summary().scf.total_energy,
        expected.hf.summary().scf.total_energy
    );
    assert_eq!(
        result.mp2.unwrap().correlation_energy,
        expected.mp2.unwrap().correlation_energy
    );

    let overridden = restored.run_hf_with_eri(CompactEri::Zeroed(2)).unwrap();
    assert_eq!(overridden.ao_eri()[(0, 0, 0, 0)], 0.0);
    let repeated = restored.execute().unwrap();
    let hf_only = restored.run_hf().unwrap();
    let pointer = &result.hf.ao_eri()[(0, 0, 0, 0)] as *const f64;
    assert_eq!(pointer, &repeated.hf.ao_eri()[(0, 0, 0, 0)] as *const f64);
    assert_eq!(pointer, &hf_only.ao_eri()[(0, 0, 0, 0)] as *const f64);
    assert_eq!(std::fs::read(path).unwrap(), archive_bytes);
}

#[test]
fn missing_eri_are_computed_once_and_retained_for_normal_execution() {
    let (geometry, basis) = inputs(1.4);
    let original = CalculationBuilder::new(&geometry, &basis)
        .prepare()
        .unwrap();
    let expected = original.execute().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("inputs-only.rustiq");
    RustiQData::from_calculation(&original)
        .unwrap()
        .write(&path)
        .unwrap();
    let archive_bytes = std::fs::read(&path).unwrap();
    let restored = RustiQData::open(&path)
        .unwrap()
        .prepare_calculation()
        .unwrap();
    let mut decisions = Vec::new();
    let mut execute = || {
        restored
            .execute_with_events(|event| {
                if let CalculationEvent::ArtifactReuse(event) = event {
                    decisions.push(event.decision);
                }
            })
            .unwrap()
    };
    let first = execute();
    let second = execute();
    assert_eq!(
        decisions,
        [
            ArtifactReuseDecision::Missing,
            ArtifactReuseDecision::Reused
        ]
    );
    assert_eq!(
        first.hf.summary().scf.total_energy,
        expected.hf.summary().scf.total_energy
    );
    assert_eq!(
        &first.hf.ao_eri()[(0, 0, 0, 0)] as *const f64,
        &second.hf.ao_eri()[(0, 0, 0, 0)] as *const f64,
    );
    assert_eq!(std::fs::read(path).unwrap(), archive_bytes);
}

#[test]
fn incompatible_artifacts_fall_back_to_the_current_calculation_and_local_cache() {
    let (source_geometry, basis) = inputs(1.4);
    let source_calculation = CalculationBuilder::new(&source_geometry, &basis)
        .prepare()
        .unwrap();
    let mut source = RustiQData::from_calculation(&source_calculation).unwrap();
    source.set_eri(CompactEri::Zeroed(2)).unwrap();
    let (geometry, _) = inputs(1.6);
    let expected = CalculationBuilder::new(&geometry, &basis)
        .execute()
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let cache_root = directory.path().join("cache");
    let calculation = CalculationBuilder::new(&geometry, &basis)
        .with_eri_cache(EriCache::new(&cache_root))
        .prepare()
        .unwrap()
        .with_reuse_data(source)
        .unwrap();
    let mut decisions = Vec::new();
    let mut cache_events = Vec::new();
    let result = calculation
        .execute_with_events(|event| match event {
            CalculationEvent::ArtifactReuse(event) => decisions.push(event.decision),
            CalculationEvent::EriCache(event) => cache_events.push(event.action),
            _ => {}
        })
        .unwrap();
    assert_eq!(decisions, [ArtifactReuseDecision::Incompatible]);
    assert_eq!(
        cache_events,
        [rustiq_core::calculation::EriCacheAction::Stored]
    );
    assert_eq!(
        result.hf.summary().scf.total_energy,
        expected.hf.summary().scf.total_energy
    );
    assert!(result.hf.ao_eri()[(0, 0, 0, 0)] > 0.0);
}

#[test]
fn explicit_portable_eri_take_precedence_over_local_cache_and_computation() {
    let (geometry, basis) = inputs(1.4);
    let original = CalculationBuilder::new(&geometry, &basis)
        .prepare()
        .unwrap();
    let mut source = RustiQData::from_calculation(&original).unwrap();
    source.set_eri(CompactEri::Zeroed(2)).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let cache_root = directory.path().join("unused-cache");
    let calculation = CalculationBuilder::new(&geometry, &basis)
        .with_eri_cache(EriCache::new(&cache_root))
        .prepare()
        .unwrap()
        .with_reuse_data(source)
        .unwrap();
    let result = calculation.execute().unwrap();
    assert_eq!(result.hf.ao_eri()[(0, 0, 0, 0)], 0.0);
    assert!(!cache_root.exists());
}

#[test]
fn restored_open_shell_calculation_preserves_requested_units_and_resolved_method() {
    use rustiq_core::{
        config::{DiisConfig, HfConfig, MoleculeConfig, ResolvedHfMethod},
        molecules::units::Units,
    };
    use std::num::NonZeroU8;
    let geometry = Geometry::from_source("oh.xyz", "2\nOH\nO 0 0 0\nH 0 0 0.97\n").unwrap();
    let (_, basis) = inputs(1.4);
    let original = CalculationBuilder::new(&geometry, &basis)
        .with_molecule_config(MoleculeConfig {
            units: Units::Angstrom,
            multiplicity: NonZeroU8::new(2).unwrap().into(),
            ..Default::default()
        })
        .with_hf(HfConfig {
            diis: DiisConfig {
                enabled: true,
                ..Default::default()
            },
            ..Default::default()
        })
        .prepare()
        .unwrap();
    let expected = original.execute().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("oh.rustiq");
    RustiQData::from_calculation(&original)
        .unwrap()
        .write_with_eri(&path, expected.hf.ao_eri())
        .unwrap();
    let restored = RustiQData::open(path)
        .unwrap()
        .prepare_calculation()
        .unwrap();
    assert_eq!(restored.hf_method(), ResolvedHfMethod::Uhf);
    assert_eq!(restored.request().molecule().units, Units::Angstrom);
    assert!(restored.hf_config().diis.enabled);
    let result = restored.execute().unwrap();
    assert!(result.hf.is_converged());
    assert_eq!(
        result.hf.summary().scf.total_energy,
        expected.hf.summary().scf.total_energy
    );
}

#[test]
fn resolved_spherical_ao_components_are_restored_without_renormalization() {
    let (geometry, _) = inputs(1.4);
    let mut basis_json: serde_json::Value =
        serde_json::from_slice(include_bytes!("data/sto-3g.json")).unwrap();
    basis_json["elements"]["1"]["electron_shells"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "function_type": "gto_spherical",
            "angular_momentum": [2],
            "exponents": ["0.8"],
            "coefficients": [["1.0"]]
        }));
    let basis =
        BasisFile::from_reader(serde_json::to_vec(&basis_json).unwrap().as_slice()).unwrap();
    let original = CalculationBuilder::new(&geometry, &basis)
        .prepare()
        .unwrap();
    let restored = RustiQData::from_calculation(&original)
        .unwrap()
        .prepare_calculation()
        .unwrap();
    assert_eq!(original.get_basis().nbasis(), 12);
    assert_eq!(
        restored.get_basis().overlap_ints(),
        original.get_basis().overlap_ints()
    );
    assert_eq!(
        restored.get_basis().kinetic_ints(),
        original.get_basis().kinetic_ints()
    );
    let expected = original.execute().unwrap();
    let result = restored.execute().unwrap();
    assert_eq!(
        result.hf.summary().scf.total_energy,
        expected.hf.summary().scf.total_energy
    );
}
