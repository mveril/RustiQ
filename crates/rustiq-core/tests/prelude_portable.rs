use rustiq_core::prelude::*;

#[test]
fn portable_workflow_uses_only_the_public_prelude() {
    let geometry =
        Geometry::from_reader(std::io::Cursor::new("2\nH2 in Bohr\nH 0 0 0\nH 1.4 0 0\n")).unwrap();
    let basis = BasisFile::from_reader(&include_bytes!("data/sto-3g.json")[..]).unwrap();
    let prepared = CalculationBuilder::new(&geometry, &basis)
        .prepare()
        .unwrap();

    let hf = prepared.run_hf().unwrap();
    let eri: &CompactEri = hf.ao_eri();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("h2.rustiq");
    let mut data = RustiQData::from_calculation(&prepared).unwrap();
    data.write_with_eri(&path, eri).unwrap();

    let restored = RustiQData::open(&path)
        .unwrap()
        .prepare_calculation()
        .unwrap();
    let mut reused = false;
    let result = restored
        .execute_with_events(|event| {
            if let CalculationEvent::ArtifactReuse(event) = event {
                reused = event.decision == ArtifactReuseDecision::Reused;
                assert_eq!(event.artifact, "ao_eri");
            }
        })
        .unwrap();
    assert!(reused);
    let _: &HfOutcome = &result.hf;

    let mut strict_data = RustiQData::open(&path).unwrap();
    let transferred: CompactEri = strict_data.take_compatible_eri(&prepared).unwrap();
    let strict_result = prepared.execute_with_eri(transferred).unwrap();
    assert!(!strict_result.hf.ao_eri().is_empty());

    let _: Option<RustiQBundle> = None;
    let _: Option<ArtifactReuseEvent> = None;
    let _: Option<CalculationExecutionError> = None;
}
