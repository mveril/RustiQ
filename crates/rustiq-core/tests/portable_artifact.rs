use rustiq_core::{
    basis::BasisFile,
    calculation::CalculationBuilder,
    molecules::geometry::Geometry,
    persistence::{AoEriArtifact, CompactEri, PortableError, RustiQData},
};

#[test]
fn public_api_creates_inspects_and_recovers_a_portable_artifact() {
    let geometry = Geometry::from_source("h2.xyz", "2\nH2\nH 0 0 0\nH 1.4 0 0\n").unwrap();
    let basis = BasisFile::from_reader(&include_bytes!("data/sto-3g.json")[..]).unwrap();
    let prepared = CalculationBuilder::new(&geometry, &basis)
        .prepare()
        .unwrap();
    let mut data = RustiQData::from_calculation(&prepared).unwrap();
    let original = b"# source comments are provenance\r\n[hf]\r\nmethod = 'Auto'\r\n";
    data.add_source("../calculation.toml", original.as_slice())
        .unwrap();
    let values = vec![0.5, 1.5, 2.5, 3.5, 4.5, 5.5];
    let indices = [
        (0, 0, 0, 0),
        (1, 0, 0, 0),
        (1, 0, 1, 0),
        (1, 1, 0, 0),
        (1, 1, 1, 0),
        (1, 1, 1, 1),
    ];
    let mut eri = CompactEri::Zeroed(2);
    for (index, value) in indices.into_iter().zip(&values) {
        eri[index] = *value;
    }
    data.set::<AoEriArtifact>(eri).unwrap();
    assert_eq!(
        data.artifact_representations().collect::<Vec<_>>(),
        vec![("ao_eri", "rustiq-compact-eri-v1")]
    );
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("h2.rustiq");
    data.write(&path).unwrap();
    assert!(matches!(
        data.write(&path),
        Err(PortableError::AlreadyExists)
    ));
    let mut restored = RustiQData::open(&path).unwrap();
    assert_eq!(
        restored.request().unwrap().hf().method.value,
        rustiq_core::config::HfMethod::Auto
    );
    assert_eq!(
        restored.request().unwrap().molecule().units,
        rustiq_core::molecules::units::Units::Bohr
    );
    let source = restored.sources().next().unwrap();
    assert_eq!(source.original_name(), "../calculation.toml");
    assert_eq!(source.bytes(), original);
    let context = restored.calculation().unwrap();
    assert_eq!(context.charge(), 0);
    assert_eq!(context.multiplicity(), 1);
    let ao = context.basis().next().unwrap();
    assert_eq!(ao.center(), [0.0; 3]);
    let component = ao.components().next().unwrap();
    assert_eq!(component.angular_momentum(), [0; 3]);
    assert_eq!(component.primitives().len(), 3);
    assert_eq!(restored.producer().0, "RustiQ");
    assert!(restored.eri_is_compatible(&prepared));
    let eri = restored.get::<AoEriArtifact>().unwrap().unwrap();
    for (index, value) in indices.into_iter().zip(values) {
        assert_eq!(eri[index], value);
    }
}
