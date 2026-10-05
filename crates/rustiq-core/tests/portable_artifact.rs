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

#[test]
fn public_bundle_api_preserves_restructured_options_for_multiple_calculations() {
    use rustiq_core::{
        config::{
            validated::{DiisSize, NonNegativeFiniteF64},
            DiisConfig, HfConfig, IntegralConfig, OrthogonalizationConfig,
        },
        persistence::RustiQBundle,
    };
    let geometry = Geometry::from_source("h2.xyz", "2\nH2\nH 0 0 0\nH 1.4 0 0\n").unwrap();
    let basis = BasisFile::from_reader(&include_bytes!("data/sto-3g.json")[..]).unwrap();
    let prepared = CalculationBuilder::new(&geometry, &basis)
        .with_hf(HfConfig {
            diis: DiisConfig {
                enabled: true,
                max_history: DiisSize::try_new(9).unwrap().into(),
            },
            orthogonalization: OrthogonalizationConfig {
                linear_dependency_threshold: NonNegativeFiniteF64::try_new(1e-7).unwrap().into(),
            },
            ..Default::default()
        })
        .with_integrals(IntegralConfig {
            schwarz_threshold: None.into(),
        })
        .prepare()
        .unwrap();
    let mut bundle = RustiQBundle::new(vec![
        RustiQData::from_calculation(&prepared).unwrap(),
        RustiQData::from_calculation(&prepared).unwrap(),
    ])
    .unwrap();
    bundle
        .add_source("input.ncl", b"opaque input".as_slice())
        .unwrap();
    bundle.calculations_mut()[1]
        .set::<AoEriArtifact>(CompactEri::Zeroed(2))
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bundle.rustiq");
    bundle.write(&path).unwrap();
    let mut reopened = RustiQBundle::open(&path).unwrap();
    assert_eq!(reopened.sources().next().unwrap().bytes(), b"opaque input");
    for entry in reopened.calculations() {
        let request = entry.request().unwrap();
        assert!(request.hf().diis.enabled);
        assert_eq!(request.hf().diis.max_history.value.into_inner(), 9);
        assert_eq!(
            request
                .hf()
                .orthogonalization
                .linear_dependency_threshold
                .value
                .into_inner(),
            1e-7
        );
        assert!(request.integrals().schwarz_threshold.value.is_none());
        assert_eq!(entry.calculation().unwrap().diis_size(), Some(9));
        assert_eq!(
            entry.calculation().unwrap().linear_dependency_threshold(),
            1e-7
        );
        assert!(entry
            .calculation()
            .unwrap()
            .eri_schwarz_threshold()
            .is_none());
    }
    assert!(reopened.calculations_mut()[0]
        .get::<AoEriArtifact>()
        .unwrap()
        .is_none());
    assert!(reopened.calculations_mut()[1]
        .get::<AoEriArtifact>()
        .unwrap()
        .is_some());
}
