use rustiq_core::{
    basis::BasisFile,
    calculation::CalculationBuilder,
    molecules::geometry::Geometry,
    persistence::{AoEriArtifact, CompactEri, PortableError, RustiQData},
};

#[test]
#[allow(
    clippy::float_cmp,
    reason = "Configuration and portable round trips must preserve literal values and identical execution results exactly"
)]
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
#[allow(
    clippy::float_cmp,
    reason = "Configuration and portable round trips must preserve literal values and identical execution results exactly"
)]
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

#[test]
#[allow(
    clippy::float_cmp,
    reason = "Configuration and portable round trips must preserve literal values and identical execution results exactly"
)]
fn borrowed_output_and_owned_input_preserve_eri_and_mp2() {
    use rustiq_core::{
        calculation::CalculationExecution, config::Mp2Config, persistence::EriCache,
    };
    let geometry = Geometry::from_source("h2.xyz", "2\nH2\nH 0 0 0\nH 1.4 0 0\n").unwrap();
    let basis = BasisFile::from_reader(&include_bytes!("data/sto-3g.json")[..]).unwrap();
    let prepared = CalculationBuilder::new(&geometry, &basis)
        .with_mp2(Mp2Config::default())
        .prepare()
        .unwrap();
    let ordinary = prepared.execute().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("h2.rustiq");
    let mut data = RustiQData::from_calculation(&prepared).unwrap();
    data.add_source("input.xyz", b"provenance".as_slice())
        .unwrap();
    let tensor = ordinary.hf.ao_eri();
    let pointer = &raw const tensor[(0, 0, 0, 0)];
    data.write_with_eri(&path, tensor).unwrap();
    assert_eq!(pointer, &raw const ordinary.hf.ao_eri()[(0, 0, 0, 0)]);
    assert!(data.get::<AoEriArtifact>().unwrap().is_none());
    let mut restored = RustiQData::open(&path).unwrap();
    assert_eq!(restored.sources().len(), 1);
    assert_eq!(restored.calculation().unwrap().charge(), 0);
    let cache_root = directory.path().join("unused-cache");
    let cached = CalculationBuilder::new(&geometry, &basis)
        .with_mp2(Mp2Config::default())
        .with_eri_cache(EriCache::new(&cache_root))
        .prepare()
        .unwrap();
    let eri = restored.take_compatible_eri(&cached).unwrap();
    let pointer = &raw const eri[(0, 0, 0, 0)];
    let supplied = cached.execute_with_eri(eri).unwrap();
    assert_eq!(pointer, &raw const supplied.hf.ao_eri()[(0, 0, 0, 0)]);
    assert!(!cache_root.exists());
    assert_eq!(
        ordinary.hf.summary().scf.electronic_energy,
        supplied.hf.summary().scf.electronic_energy
    );
    assert_eq!(
        ordinary.mp2.unwrap().correlation_energy,
        supplied.mp2.unwrap().correlation_energy
    );
    assert!(cached.execute_with_eri(CompactEri::Zeroed(3)).is_err());
    let zero = cached.run_hf_with_eri(CompactEri::Zeroed(2)).unwrap();
    assert_eq!(zero.ao_eri()[(0, 0, 0, 0)], 0.0);
    assert!(!cache_root.exists());
}

#[test]
fn explicit_input_rejects_missing_incompatible_and_multiple_calculations() {
    use rustiq_core::{
        config::IntegralConfig,
        persistence::{ArtifactError, RustiQBundle},
    };
    let geometry = Geometry::from_source("h2.xyz", "2\nH2\nH 0 0 0\nH 1.4 0 0\n").unwrap();
    let basis = BasisFile::from_reader(&include_bytes!("data/sto-3g.json")[..]).unwrap();
    let prepared = CalculationBuilder::new(&geometry, &basis)
        .prepare()
        .unwrap();
    let mut data = RustiQData::from_calculation(&prepared).unwrap();
    assert!(matches!(
        data.take_compatible_eri(&prepared),
        Err(PortableError::Artifact(ArtifactError::Missing))
    ));
    data.set_eri(CompactEri::Zeroed(2)).unwrap();
    let changed = CalculationBuilder::new(&geometry, &basis)
        .with_integrals(IntegralConfig {
            schwarz_threshold: None.into(),
        })
        .prepare()
        .unwrap();
    assert!(matches!(
        data.take_compatible_eri(&changed),
        Err(PortableError::IncompatibleEri)
    ));
    assert!(data.read_eri().is_ok());
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("multi.rustiq");
    RustiQBundle::new(vec![data, RustiQData::from_calculation(&prepared).unwrap()])
        .unwrap()
        .write(&path)
        .unwrap();
    assert!(RustiQData::open(path).is_err());
}

#[test]
fn explicit_input_reports_corrupt_and_unsupported_eri() {
    use rustiq_core::persistence::ArtifactError;
    use std::{
        fs::File,
        io::{Read, Write},
    };
    use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};
    let geometry = Geometry::from_source("h2.xyz", "2\nH2\nH 0 0 0\nH 1.4 0 0\n").unwrap();
    let basis = BasisFile::from_reader(&include_bytes!("data/sto-3g.json")[..]).unwrap();
    let prepared = CalculationBuilder::new(&geometry, &basis)
        .prepare()
        .unwrap();
    let hf = prepared.run_hf().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let original = directory.path().join("original.rustiq");
    RustiQData::from_calculation(&prepared)
        .unwrap()
        .write_with_eri(&original, hf.ao_eri())
        .unwrap();
    for unsupported in [false, true] {
        let path = directory
            .path()
            .join(format!("changed-{unsupported}.rustiq"));
        let mut archive = ZipArchive::new(File::open(&original).unwrap()).unwrap();
        let mut writer = ZipWriter::new(File::create(&path).unwrap());
        for index in 0..archive.len() {
            let mut member = archive.by_index(index).unwrap();
            let name = member.name().to_owned();
            let mut bytes = Vec::new();
            member.read_to_end(&mut bytes).unwrap();
            if unsupported && name == "manifest.json" {
                let mut manifest: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                manifest["calculations"][0]["artifacts"]["ao_eri"]["representation"] =
                    "future-eri-v2".into();
                bytes = serde_json::to_vec(&manifest).unwrap();
            } else if !unsupported && name.ends_with("ao-eri.npy") {
                *bytes.last_mut().unwrap() ^= 1;
            }
            writer
                .start_file(name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(&bytes).unwrap();
        }
        writer.finish().unwrap();
        let mut data = RustiQData::open(path).unwrap();
        let error = data.take_compatible_eri(&prepared).unwrap_err();
        if unsupported {
            assert!(matches!(
                error,
                PortableError::Artifact(ArtifactError::UnsupportedRepresentation(_))
            ));
        } else {
            assert!(matches!(
                error,
                PortableError::Artifact(ArtifactError::IntegrityMismatch(_))
            ));
        }
    }
}
