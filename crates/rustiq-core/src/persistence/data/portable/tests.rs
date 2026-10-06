#![allow(
    unknown_lints,
    reason = "assert_is_empty is only available starting with Clippy 1.99"
)]
#![allow(
    clippy::assert_is_empty,
    reason = "Idiomatic is_empty assertions express the test intent without typed empty collections"
)]

use super::super::super::{sha256, AoEriArtifact};
use super::*;
use crate::{
    calculation::CalculationBuilder,
    config::{HfConfig, HfMethod, MoleculeConfig, Mp2Config},
    eri::CompactEri,
    molecules::geometry::Geometry,
    test_utils::load_minimal_basis_file,
};
use std::{
    fs::{self, File},
    io::{Read, Write},
    num::NonZeroU8,
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

const CALCULATION_PATH: &str = "calculations/calculation-0/calculation.json";
const REQUEST_PATH: &str = "calculations/calculation-0/request.json";
const AO_ERI_PATH: &str = "calculations/calculation-0/arrays/integrals/ao-eri.npy";

fn calculation() -> PreparedCalculation {
    let geometry = Geometry::from_source("private.xyz", "2\nH2\nH 0 0 0\nH 1.4 0 0\n").unwrap();
    CalculationBuilder::new(&geometry, &load_minimal_basis_file())
        .prepare()
        .unwrap()
}
fn data() -> RustiQData {
    let mut data = RustiQData::from_calculation(&calculation()).unwrap();
    data.set_eri(CompactEri::from_ordered_values(2, vec![0.5, 1.5, 2.5, 3.5, 4.5, 5.5]).unwrap())
        .unwrap();
    data
}

#[test]
fn borrowed_eri_writer_rejects_multi_calculation_input() {
    let first = RustiQData::from_calculation(&calculation()).unwrap();
    let second = RustiQData::from_calculation(&calculation()).unwrap();
    let mut calculations = [first, second];
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("invalid.rustiq");

    let error = super::super::bundle::write_bundle_with_eri(
        &mut calculations,
        &[],
        &destination,
        Some(&CompactEri::Zeroed(2)),
    )
    .unwrap_err();

    assert!(matches!(error, PortableError::InvalidArchive(_)));
    assert!(!destination.exists());
}

fn source_fixture() -> Vec<u8> {
    include_str!("../../../../tests/data/persistence/source-original-v1.toml.hex")
        .trim()
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
fn members(path: &Path) -> Vec<(String, Vec<u8>)> {
    let mut archive = ZipArchive::new(File::open(path).unwrap()).unwrap();
    (0..archive.len())
        .map(|i| {
            let mut member = archive.by_index(i).unwrap();
            let name = member.name().to_owned();
            let mut bytes = Vec::new();
            member.read_to_end(&mut bytes).unwrap();
            (name, bytes)
        })
        .collect()
}
fn write_members(path: &Path, members: &[(String, Vec<u8>)]) {
    let mut writer = ZipWriter::new(File::create(path).unwrap());
    for (name, bytes) in members {
        writer
            .start_file(
                name,
                SimpleFileOptions::default()
                    .compression_method(CompressionMethod::Stored)
                    .large_file(true),
            )
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap();
}
fn edit_json(
    members: &mut [(String, Vec<u8>)],
    name: &str,
    edit: impl FnOnce(&mut serde_json::Value),
) {
    let bytes = &mut members.iter_mut().find(|(n, _)| n == name).unwrap().1;
    let mut json = serde_json::from_slice(bytes).unwrap();
    edit(&mut json);
    *bytes = serde_json::to_vec(&json).unwrap();
}
fn refresh_snapshot_digest(members: &mut [(String, Vec<u8>)]) {
    let snapshot = &members
        .iter()
        .find(|(n, _)| n == CALCULATION_PATH)
        .unwrap()
        .1;
    let (size, digest) = (snapshot.len(), sha256(snapshot).to_string());
    edit_json(members, "manifest.json", |m| {
        m["calculations"][0]["calculation"]["size"] = size.into();
        m["calculations"][0]["calculation"]["digest"] = digest.into();
    });
}
fn refresh_eri_digest(members: &mut [(String, Vec<u8>)]) {
    let payload = &members.iter().find(|(n, _)| n == AO_ERI_PATH).unwrap().1;
    let (size, digest) = (payload.len(), sha256(payload).to_string());
    edit_json(members, "manifest.json", |m| {
        m["calculations"][0]["artifacts"]["ao_eri"]["size"] = size.into();
        m["calculations"][0]["artifacts"]["ao_eri"]["digest"] = digest.into();
    });
}

#[test]
fn rust_round_trip_is_lazy_self_describing_and_reproducible() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.rustiq");
    let second = dir.path().join("second.rustiq");
    data().write(&first).unwrap();
    data().write(&second).unwrap();
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let mut restored = RustiQData::open(&first).unwrap();
    assert!(restored.ao_eri.is_none());
    let context = restored.calculation().unwrap();
    assert_eq!(
        context.atoms().collect::<Vec<_>>(),
        vec![(1, [0.0, 0.0, 0.0]), (1, [1.4, 0.0, 0.0])]
    );
    assert_eq!(context.hf_method(), crate::config::ResolvedHfMethod::Rhf);
    assert_eq!(context.basis().len(), 2);
    assert!(restored.eri_is_compatible(&calculation()));
    assert_eq!(
        restored.read_eri().unwrap().ordered_values(),
        &[0.5, 1.5, 2.5, 3.5, 4.5, 5.5]
    );
    let pointer = std::ptr::from_ref(restored.read_eri().unwrap());
    assert_eq!(std::ptr::from_ref(restored.read_eri().unwrap()), pointer);
    let zip = ZipArchive::new(File::open(&first).unwrap()).unwrap();
    assert_eq!(zip.len(), 4);
    let request = members(&first)
        .into_iter()
        .find(|(n, _)| n == REQUEST_PATH)
        .unwrap()
        .1;
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request).unwrap(),
        serde_json::from_str::<serde_json::Value>(include_str!(
            "../../../../tests/data/persistence/request-h2-v1.json"
        ))
        .unwrap()
    );
    assert_eq!(restored.sources().len(), 0);
    let snapshot = members(&first)
        .into_iter()
        .find(|(n, _)| n == CALCULATION_PATH)
        .unwrap()
        .1;
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&snapshot).unwrap(),
        serde_json::from_str::<serde_json::Value>(include_str!(
            "../../../../tests/data/persistence/calculation-h2-v1.json"
        ))
        .unwrap()
    );
}

#[test]
#[allow(
    clippy::float_cmp,
    reason = "Persistence and copy-on-write tests require exact preservation of stored values"
)]
fn normalized_request_round_trip_keeps_auto_angstrom_defaults_and_requested_options() {
    use crate::config::{DensityGuessConfig, MemoryLimit, RandomGuessConfig};
    use crate::molecules::units::Units;
    let geometry =
        Geometry::from_source("input.xyz", "2\nignored comment\nH 0 0 0\nH 0.74 0 0\n").unwrap();
    let basis = load_minimal_basis_file();
    let defaulted = CalculationBuilder::new(&geometry, &basis)
        .prepare()
        .unwrap();
    let explicit = CalculationBuilder::new(&geometry, &basis)
        .with_hf(HfConfig::default())
        .with_molecule_config(MoleculeConfig::default())
        .prepare()
        .unwrap();
    assert_eq!(
        serde_json::to_value(RequestSnapshot::from_request(defaulted.request()).unwrap()).unwrap(),
        serde_json::to_value(RequestSnapshot::from_request(explicit.request()).unwrap()).unwrap()
    );
    let prepared = CalculationBuilder::new(&geometry, &basis)
        .with_basis_label("portable-basis-label")
        .with_molecule_config(MoleculeConfig {
            units: Units::Angstrom,
            ..Default::default()
        })
        .with_hf(HfConfig {
            guess: DensityGuessConfig::Random {
                config: RandomGuessConfig::default(),
            }
            .into(),
            ..Default::default()
        })
        .with_mp2(Mp2Config {
            frozen_orbitals: 1.into(),
            memory_limit: MemoryLimit::Fixed(bytesize::ByteSize::mib(42)).into(),
        })
        .prepare()
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("round-trip.rustiq");
    RustiQData::from_calculation(&prepared)
        .unwrap()
        .write(&path)
        .unwrap();
    let restored = RustiQData::open(&path).unwrap();
    let request = restored.request().unwrap();
    assert_eq!(request.hf().method.value, HfMethod::Auto);
    assert_eq!(request.molecule().units, Units::Angstrom);
    assert_eq!(request.geometry().atoms[1].position[0], 0.74);
    assert_eq!(request.basis_name(), "portable-basis-label");
    assert_eq!(
        request.mp2().unwrap().memory_limit.value,
        prepared.request().mp2().unwrap().memory_limit.value
    );
    assert!(request.geometry().comment.is_empty());
    let DensityGuessConfig::Random { config } = request.hf().guess.value else {
        panic!()
    };
    assert_eq!(config.random.seed, None);
    let context = restored.calculation().unwrap();
    assert_eq!(context.hf_method(), crate::config::ResolvedHfMethod::Rhf);
    assert!(context.atoms().nth(1).unwrap().1[0] > 1.0);
    let DensityGuessConfig::Random { config } = context.density_guess() else {
        panic!()
    };
    assert!(config.random.seed.is_some());
    assert_eq!(
        serde_json::to_value(RequestSnapshot::from_request(request).unwrap()).unwrap(),
        serde_json::to_value(RequestSnapshot::from_request(prepared.request()).unwrap()).unwrap()
    );
}

#[test]
fn opaque_sources_preserve_exact_bytes_without_affecting_scientific_state() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.rustiq");
    let output = dir.path().join("copied.rustiq");
    let mut original = data();
    let sources: [(&str, &[u8]); 2] = [
        (
            "../../private/calculation.toml",
            b"# preserve comments\r\n[hf]\r\nmethod = 'Auto'\r\n",
        ),
        (
            "C:\\private\\molecule.xyz",
            b"2\r\noriginal comment\r\nH 0.000 0 0\r\nH 1.4000 0 0\r\n\xff",
        ),
    ];
    for (name, bytes) in sources {
        original.add_source(name, bytes).unwrap();
    }
    original.write(&path).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
    }
    let mut restored = RustiQData::open(&path).unwrap();
    assert!(restored.ao_eri.is_none());
    assert!(restored.eri_is_compatible(&calculation()));
    for (source, (name, bytes)) in restored.sources().zip(sources) {
        assert_eq!(source.original_name(), name);
        assert_eq!(source.bytes(), bytes);
    }
    let archive_members = members(&path);
    assert!(archive_members.iter().any(|(name, _)| name == "sources/0"));
    assert!(!archive_members
        .iter()
        .any(|(name, _)| name.contains("private")));
    restored.write(&output).unwrap();
    assert_eq!(members(&output), archive_members);
    assert_eq!(members(&path), archive_members);
}

#[test]
fn request_and_source_integrity_versions_and_limits_are_checked_on_open() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("good.rustiq");
    let mut original = data();
    original
        .add_source("input.toml", b"original bytes".as_slice())
        .unwrap();
    original.write(&path).unwrap();
    let original = members(&path);
    let broken = dir.path().join("broken.rustiq");
    for (target, field, value) in [
        (
            "manifest.json",
            "/calculations/0/request",
            serde_json::Value::Null,
        ),
        (
            "manifest.json",
            "/calculations/0/request/path",
            serde_json::json!(CALCULATION_PATH),
        ),
        (
            "manifest.json",
            "/calculations/0/request/version",
            serde_json::json!(99),
        ),
        (
            "manifest.json",
            "/calculations/0/request/size",
            serde_json::json!(MAX_REQUEST_BYTES + 1),
        ),
        (
            "manifest.json",
            "/sources/0/path",
            serde_json::json!("../input.toml"),
        ),
        ("manifest.json", "/sources/0/version", serde_json::json!(99)),
        (
            "manifest.json",
            "/sources/0/size",
            serde_json::json!(MAX_SOURCE_BYTES + 1),
        ),
        (REQUEST_PATH, "/version", serde_json::json!(99)),
        (REQUEST_PATH, "/units", serde_json::json!("invalid")),
        (REQUEST_PATH, "/hf/method", serde_json::json!("invalid")),
        (REQUEST_PATH, "/hf/max_iterations", serde_json::json!(0)),
        (REQUEST_PATH, "/hf/diis_size", serde_json::json!(1)),
        (
            REQUEST_PATH,
            "/hf/convergence_threshold",
            serde_json::json!(-1),
        ),
        (
            REQUEST_PATH,
            "/atoms/0/atomic_number",
            serde_json::json!(119),
        ),
        (REQUEST_PATH, "/atoms/0/position/0", serde_json::json!(12)),
    ] {
        let mut contents = original.clone();
        edit_json(&mut contents, target, |json| {
            *json.pointer_mut(field).unwrap() = value;
        });
        if target == REQUEST_PATH {
            let bytes = &contents
                .iter()
                .find(|(name, _)| name == REQUEST_PATH)
                .unwrap()
                .1;
            let (size, digest) = (bytes.len(), sha256(bytes).to_string());
            edit_json(&mut contents, "manifest.json", |m| {
                m["calculations"][0]["request"]["size"] = size.into();
                m["calculations"][0]["request"]["digest"] = digest.into();
            });
        }
        write_members(&broken, &contents);
        assert!(RustiQData::open(&broken).is_err(), "{target} {field}");
    }
    for name in [REQUEST_PATH, "sources/0"] {
        let mut contents = original.clone();
        contents.iter_mut().find(|(n, _)| n == name).unwrap().1[0] ^= 1;
        write_members(&broken, &contents);
        assert!(matches!(
            RustiQData::open(&broken),
            Err(PortableError::Artifact(ArtifactError::IntegrityMismatch(_)))
        ));
        contents.retain(|(n, _)| n != name);
        write_members(&broken, &contents);
        assert!(RustiQData::open(&broken).is_err());
    }
}

#[test]
fn empty_artifacts_uhf_and_mp2_context_round_trip() {
    let geometry = Geometry::from_source("H.xyz", "1\nH\nH 0 0 0\n").unwrap();
    let basis = load_minimal_basis_file();
    let prepared = CalculationBuilder::new(&geometry, &basis)
        .with_molecule_config(MoleculeConfig {
            multiplicity: NonZeroU8::new(2).unwrap().into(),
            ..Default::default()
        })
        .with_hf(HfConfig {
            method: HfMethod::Uhf.into(),
            diis: crate::config::DiisConfig {
                enabled: true,
                ..Default::default()
            },
            ..Default::default()
        })
        .with_mp2(Mp2Config::default())
        .prepare()
        .unwrap();
    let mut data = RustiQData::from_calculation(&prepared).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("h.rustiq");
    data.write(&path).unwrap();
    let mut restored = RustiQData::open(&path).unwrap();
    assert!(restored.get::<AoEriArtifact>().unwrap().is_none());
    let context = restored.calculation().unwrap();
    assert_eq!(context.multiplicity(), 2);
    assert_eq!(context.hf_method(), crate::config::ResolvedHfMethod::Uhf);
    assert_eq!(context.diis_size(), Some(6));
    assert_eq!(context.mp2_frozen_orbitals(), Some(0));
}

#[test]
fn compatibility_depends_on_effective_integral_inputs_only() {
    let prepared = calculation();
    let mut data = data();
    let mp2 = CalculationBuilder::new(prepared.get_molecule(), &load_minimal_basis_file())
        .with_mp2(Mp2Config::default())
        .prepare()
        .unwrap();
    assert!(data.eri_is_compatible(&mp2));
    let changed = CalculationBuilder::new(prepared.get_molecule(), &load_minimal_basis_file())
        .with_integrals(crate::config::IntegralConfig {
            schwarz_threshold: None.into(),
        })
        .prepare()
        .unwrap();
    assert!(!data.eri_is_compatible(&changed));
    data.ao_eri = None;
    assert!(!data.eri_is_compatible(&prepared));
}

#[test]
fn publication_never_overwrites_and_concurrent_writer_has_one_winner() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("target.rustiq");
    fs::write(&path, b"user data").unwrap();
    assert!(matches!(
        data().write(&path),
        Err(PortableError::AlreadyExists)
    ));
    assert_eq!(fs::read(&path).unwrap(), b"user data");
    fs::remove_file(&path).unwrap();
    let barrier = std::sync::Barrier::new(2);
    let results = std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            let mut data = data();
            barrier.wait();
            data.write(&path)
        });
        let second = scope.spawn(|| {
            let mut data = data();
            barrier.wait();
            data.write(&path)
        });
        [first.join().unwrap(), second.join().unwrap()]
    });
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Err(PortableError::AlreadyExists)))
            .count(),
        1
    );
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    RustiQData::open(&path).unwrap().read_eri().unwrap();
}

#[test]
fn corrupt_payload_is_lazy_but_cannot_be_read_or_republished() {
    use crate::calculation::{CalculationError, CalculationExecution};
    let dir = tempfile::tempdir().unwrap();
    let original = dir.path().join("original.rustiq");
    data().write(&original).unwrap();
    let mut contents = members(&original);
    *contents
        .iter_mut()
        .find(|(n, _)| n == AO_ERI_PATH)
        .unwrap()
        .1
        .last_mut()
        .unwrap() ^= 1;
    let corrupt = dir.path().join("corrupt.rustiq");
    write_members(&corrupt, &contents);
    let portable_calculation = RustiQData::open(&corrupt)
        .unwrap()
        .prepare_calculation()
        .unwrap();
    let error = portable_calculation.execute().unwrap_err();
    assert!(matches!(
        error.cause(),
        CalculationError::Artifact(ArtifactError::IntegrityMismatch(_))
    ));
    let mut restored = RustiQData::open(&corrupt).unwrap();
    assert!(restored.take_compatible_eri(&calculation()).is_err());
    assert!(restored.read_eri().is_err());
    let output = dir.path().join("output.rustiq");
    assert!(restored.write(&output).is_err());
    assert!(!output.exists());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
}

#[test]
fn unknown_artifacts_survive_verified_streaming_copy() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.rustiq");
    data().write(&first).unwrap();
    let mut contents = members(&first);
    let opaque = b"future payload".to_vec();
    contents.push((
        "calculations/calculation-0/arrays/future.bin".into(),
        opaque.clone(),
    ));
    edit_json(&mut contents, "manifest.json", |m| {
        m["calculations"][0]["artifacts"]["future"] = serde_json::json!({"path":"calculations/calculation-0/arrays/future.bin","size":opaque.len(),"digest":sha256(&opaque),"representation":"future-v99","attributes":{"opaque":17}});
    });
    let source = dir.path().join("source.rustiq");
    write_members(&source, &contents);
    let mut restored = RustiQData::open(&source).unwrap();
    let output = dir.path().join("copy.rustiq");
    restored.write(&output).unwrap();
    assert!(restored.ao_eri.is_none());
    assert!(members(&output).contains(&(
        "calculations/calculation-0/arrays/future.bin".into(),
        opaque
    )));
    assert_eq!(
        RustiQData::open(&output).unwrap().manifest.artifacts["future"],
        restored.manifest.artifacts["future"]
    );
    assert_eq!(members(&source), contents);
}

#[test]
fn rejects_invalid_context_manifest_and_artifact_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let original = dir.path().join("original.rustiq");
    data().write(&original).unwrap();
    let original = members(&original);
    for (target, field, value) in [
        ("manifest.json", "/format_version", serde_json::json!(999)),
        (
            "manifest.json",
            "/calculations/0/scientific_identity/version",
            serde_json::json!(99),
        ),
        (
            "manifest.json",
            "/calculations/0/artifacts/ao_eri/attributes/basis_functions",
            serde_json::json!(3),
        ),
        (
            "manifest.json",
            "/calculations/0/artifacts/ao_eri/attributes/computation_version",
            serde_json::json!(99),
        ),
        (
            "manifest.json",
            "/calculations/0/artifacts/ao_eri/path",
            serde_json::json!("CALCULATION.JSON"),
        ),
        (
            "manifest.json",
            "/calculations/0/artifacts/ao_eri/size",
            serde_json::json!(1),
        ),
        (CALCULATION_PATH, "/version", serde_json::json!(99)),
        (CALCULATION_PATH, "/units", serde_json::json!("angstrom")),
        (CALCULATION_PATH, "/charge", serde_json::json!(100)),
        (CALCULATION_PATH, "/multiplicity", serde_json::json!(0)),
        (
            CALCULATION_PATH,
            "/atoms/0/position/0",
            serde_json::json!(5.0),
        ),
        (
            CALCULATION_PATH,
            "/basis/0/components/0/primitives/0/exponent",
            serde_json::json!(-1.0),
        ),
        (CALCULATION_PATH, "/hf/diis_size", serde_json::json!(1)),
        (
            CALCULATION_PATH,
            "/hf/convergence_threshold",
            serde_json::json!(0),
        ),
    ] {
        let mut contents = original.clone();
        edit_json(&mut contents, target, |j| {
            *j.pointer_mut(field).unwrap() = value;
        });
        if target == CALCULATION_PATH {
            refresh_snapshot_digest(&mut contents);
        }
        let path = dir.path().join("invalid.rustiq");
        write_members(&path, &contents);
        assert!(RustiQData::open(&path).is_err(), "{target} {field}");
    }
    let mut contents = original;
    contents.retain(|(n, _)| n != CALCULATION_PATH);
    let path = dir.path().join("missing.rustiq");
    write_members(&path, &contents);
    assert!(RustiQData::open(path).is_err());
}

#[test]
fn rejects_unsafe_member_paths_and_local_header_disagreement() {
    let dir = tempfile::tempdir().unwrap();
    for name in [
        "../evil",
        "/evil",
        "C:/evil",
        "a\\evil",
        "a/CON",
        "a/",
        "a//b",
        "a/./b",
        "a/../b",
        "a/b.",
        "a/b ",
        "a/b?",
        "manifest.json/child",
    ] {
        let path = dir.path().join("bad.rustiq");
        let mut contents = vec![("manifest.json".into(), b"{}".to_vec())];
        contents.push((name.into(), vec![]));
        write_members(&path, &contents);
        assert!(RustiQData::open(&path).is_err(), "{name}");
    }
    let path = dir.path().join("good.rustiq");
    data().write(&path).unwrap();
    let original = fs::read(&path).unwrap();
    for offset in [30, 6, 8] {
        let mut bytes = original.clone();
        bytes[offset] ^= 1;
        let invalid = dir.path().join("invalid.rustiq");
        fs::write(&invalid, bytes).unwrap();
        assert!(RustiQData::open(&invalid).is_err());
    }
    for len in [0, 10, original.len() / 2, original.len() - 1] {
        let invalid = dir.path().join("truncated.rustiq");
        fs::write(&invalid, &original[..len]).unwrap();
        assert!(RustiQData::open(invalid).is_err());
    }
}

#[test]
fn rejects_duplicate_members_before_zip_library_deduplication() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("duplicate.rustiq");
    write_members(
        &path,
        &[("one.json".into(), vec![]), ("two.json".into(), vec![])],
    );
    let mut bytes = fs::read(&path).unwrap();
    for index in 0..bytes.len() - 8 {
        if &bytes[index..index + 8] == b"two.json" {
            bytes[index..index + 8].copy_from_slice(b"one.json");
        }
    }
    fs::write(&path, bytes).unwrap();
    assert!(RustiQData::open(&path)
        .unwrap_err()
        .to_string()
        .contains("duplicate"));
    write_members(
        &path,
        &[
            ("manifest.json".into(), vec![]),
            ("MANIFEST.JSON".into(), vec![]),
        ],
    );
    assert!(RustiQData::open(path)
        .unwrap_err()
        .to_string()
        .contains("duplicate"));
}

#[test]
fn malformed_npy_is_rejected_even_with_correct_digest() {
    let dir = tempfile::tempdir().unwrap();
    let good = dir.path().join("good.rustiq");
    data().write(&good).unwrap();
    for payload in [
        b"not npy".to_vec(),
        b"\x93NUMPY\x02\x00\xff\xff\xff\x7f".to_vec(),
    ] {
        let mut contents = members(&good);
        contents
            .iter_mut()
            .find(|(n, _)| n == AO_ERI_PATH)
            .unwrap()
            .1 = payload;
        refresh_eri_digest(&mut contents);
        let bad = dir.path().join("bad.rustiq");
        write_members(&bad, &contents);
        let mut restored = RustiQData::open(&bad).unwrap();
        assert!(restored.take_compatible_eri(&calculation()).is_err());
        assert!(restored.read_eri().is_err());
        assert!(restored.write(dir.path().join("copy.rustiq")).is_err());
    }
}

#[test]
#[ignore = "writes a portable artifact for Python interoperability"]
fn writes_archive_for_python_interoperability() {
    let path = std::env::var_os("RUSTIQ_ARCHIVE_TEST_OUTPUT").expect("output path");
    let mut data = data();
    data.add_source("../../calculation.toml", source_fixture())
        .unwrap();
    data.write(path).unwrap();
}

#[test]
fn reads_python_zip64_fixtures_with_both_npy_byte_orders() {
    for fixture in [
        include_str!("../../../../tests/data/persistence/portable-python-little-v1.rustiq.hex"),
        include_str!("../../../../tests/data/persistence/portable-python-big-v1.rustiq.hex"),
    ] {
        let bytes: Vec<u8> = fixture
            .trim()
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        assert!(bytes.windows(4).any(|w| w == b"PK\x06\x06"));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("python.rustiq");
        fs::write(&path, bytes).unwrap();
        let mut data = RustiQData::open(&path).unwrap();
        assert!(data.eri_is_compatible(&calculation()));
        assert_eq!(
            data.read_eri().unwrap().ordered_values(),
            &[0.5, 1.5, 2.5, 3.5, 4.5, 5.5]
        );
    }
}

#[test]
#[cfg(unix)]
fn zip64_handles_offsets_beyond_four_gib_without_allocating_large_payloads() {
    use std::io::{Seek, SeekFrom};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sparse.rustiq");
    let mut file = File::create(&path).unwrap();
    file.seek(SeekFrom::Start(u64::from(u32::MAX) + 1)).unwrap();
    super::super::bundle::write_snapshot(
        std::slice::from_mut(&mut data()),
        &[],
        Storage::create_zip(file),
    )
    .unwrap();
    assert!(fs::metadata(&path).unwrap().len() > u64::from(u32::MAX));
    let mut restored = RustiQData::open(&path).unwrap();
    assert_eq!(
        restored.read_eri().unwrap().ordered_values(),
        &[0.5, 1.5, 2.5, 3.5, 4.5, 5.5]
    );
}

#[test]
fn rejects_links_special_entries_encryption_and_oversized_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.rustiq");
    data().write(&source).unwrap();
    let original = fs::read(&source).unwrap();
    let central = original
        .windows(4)
        .position(|w| w == b"PK\x01\x02")
        .unwrap();
    for mode in [0o120_777_u32, 0o020_666, 0o040_755] {
        let mut bytes = original.clone();
        bytes[central + 38..central + 42].copy_from_slice(&(mode << 16).to_le_bytes());
        let path = dir.path().join("special.rustiq");
        fs::write(&path, bytes).unwrap();
        assert!(RustiQData::open(path).is_err());
    }
    for flags in [1_u16, 0x40, 0x2000] {
        let mut bytes = original.clone();
        bytes[central + 8..central + 10].copy_from_slice(&flags.to_le_bytes());
        let path = dir.path().join("encrypted.rustiq");
        fs::write(&path, bytes).unwrap();
        assert!(RustiQData::open(path).is_err());
    }
    let mut contents = members(&source);
    let manifest = &mut contents
        .iter_mut()
        .find(|(n, _)| n == "manifest.json")
        .unwrap()
        .1;
    manifest.resize(1024 * 1024 + 1, b' ');
    let path = dir.path().join("huge.rustiq");
    write_members(&path, &contents);
    assert!(RustiQData::open(path).is_err());
    let mut bytes = original;
    let end = bytes.len() - 22;
    bytes[end + 8..end + 10].copy_from_slice(&4097_u16.to_le_bytes());
    bytes[end + 10..end + 12].copy_from_slice(&4097_u16.to_le_bytes());
    let path = dir.path().join("many.rustiq");
    fs::write(&path, bytes).unwrap();
    assert!(RustiQData::open(path).is_err());
    let mut contents = members(&source);
    edit_json(&mut contents, "manifest.json", |m| {
        m["calculations"][0]["calculation"]["size"] = (MAX_CALCULATION_BYTES + 1).into();
    });
    let path = dir.path().join("snapshot-limit.rustiq");
    write_members(&path, &contents);
    assert!(RustiQData::open(path).is_err());
}

#[test]
fn crc_and_actual_decompressed_size_are_checked() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("valid.rustiq");
    data().write(&path).unwrap();
    // Metadata is deflated. Tamper with its declared length in the central directory.
    let mut bytes = fs::read(&path).unwrap();
    let central = bytes.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
    bytes[central + 24..central + 28].copy_from_slice(&1_u32.to_le_bytes());
    let bad = dir.path().join("size.rustiq");
    fs::write(&bad, bytes).unwrap();
    assert!(RustiQData::open(bad).is_err());
    let mut bytes = fs::read(&path).unwrap();
    bytes[central + 16] ^= 1;
    let bad = dir.path().join("crc.rustiq");
    fs::write(&bad, bytes).unwrap();
    assert!(RustiQData::open(bad).is_err());
}

#[test]
fn settings_are_resolved_and_machine_policy_is_excluded() {
    use crate::config::{
        DensityGuessConfig, GuessPerturbationConfig, MemoryLimit, RandomGuessConfig,
    };
    use crate::molecules::units::Units;
    let basis = load_minimal_basis_file();
    let geometry = Geometry::from_source(
        "source-path-must-not-leak.xyz",
        "2\nH2\nH 0 0 0\nH 0.74 0 0\n",
    )
    .unwrap();
    for guess in [
        DensityGuessConfig::Zero,
        DensityGuessConfig::CoreHamiltonian {
            perturbation: Some(GuessPerturbationConfig::default()),
        },
        DensityGuessConfig::OneElectron {
            perturbation: Some(GuessPerturbationConfig::default()),
        },
        DensityGuessConfig::Random {
            config: RandomGuessConfig::default(),
        },
    ] {
        let prepared = CalculationBuilder::new(&geometry, &basis)
            .with_molecule_config(MoleculeConfig {
                units: Units::Angstrom,
                ..Default::default()
            })
            .with_hf(HfConfig {
                guess: guess.into(),
                ..Default::default()
            })
            .with_mp2(Mp2Config {
                memory_limit: MemoryLimit::Fixed(bytesize::ByteSize::mib(42)).into(),
                ..Default::default()
            })
            .prepare()
            .unwrap();
        let data = RustiQData::from_calculation(&prepared).unwrap();
        let snapshot = &data.calculation().unwrap().0;
        let json = serde_json::to_string(snapshot).unwrap();
        for forbidden in [
            "source-path",
            "memory_limit",
            "cache",
            "span",
            "auto",
            "angstrom",
        ] {
            assert!(!json.contains(forbidden), "{forbidden}");
        }
        let restored: Snapshot = serde_json::from_str(&json).unwrap();
        restored.validate().unwrap();
        assert_eq!(snapshot.identity(), restored.identity());
        assert_eq!(
            format!(
                "{:?}",
                CalculationContext::from_snapshot(restored)
                    .unwrap()
                    .density_guess()
            ),
            format!("{:?}", prepared.hf_config().guess.value)
        );
        assert!(data.calculation().unwrap().atoms().nth(1).unwrap().1[0] > 1.0);
    }
}

#[test]
fn unsupported_representation_is_inspectable_but_not_scientifically_usable() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.rustiq");
    data().write(&source).unwrap();
    let mut contents = members(&source);
    edit_json(&mut contents, "manifest.json", |m| {
        m["calculations"][0]["artifacts"]["ao_eri"]["representation"] = "future-eri-v2".into();
    });
    let unknown = dir.path().join("unknown.rustiq");
    write_members(&unknown, &contents);
    let mut restored = RustiQData::open(&unknown).unwrap();
    assert!(restored.calculation().is_some());
    assert!(!restored.eri_is_compatible(&calculation()));
    assert!(restored.take_compatible_eri(&calculation()).is_err());
    assert!(restored.read_eri().is_err());
    restored.write(dir.path().join("copy.rustiq")).unwrap();
}

#[test]
fn rewriting_foreign_producer_updates_provenance_only() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.rustiq");
    data().write(&first).unwrap();
    let mut contents = members(&first);
    edit_json(&mut contents, "manifest.json", |m| {
        m["producer"] = serde_json::json!({"name":"Python","version":"old"});
    });
    let foreign = dir.path().join("foreign.rustiq");
    write_members(&foreign, &contents);
    let mut restored = RustiQData::open(&foreign).unwrap();
    assert_eq!(restored.producer(), ("Python", "old"));
    let output = dir.path().join("rewritten.rustiq");
    restored.write(&output).unwrap();
    assert!(restored.ao_eri.is_none());
    let mut rewritten = RustiQData::open(&output).unwrap();
    assert_eq!(rewritten.producer(), ("RustiQ", env!("CARGO_PKG_VERSION")));
    assert_eq!(
        rewritten.manifest.scientific_identity,
        restored.manifest.scientific_identity
    );
    assert_eq!(rewritten.manifest.artifacts, restored.manifest.artifacts);
    assert!(rewritten.eri_is_compatible(&calculation()));
    assert_eq!(
        rewritten.read_eri().unwrap().ordered_values(),
        &[0.5, 1.5, 2.5, 3.5, 4.5, 5.5]
    );
    assert_eq!(members(&foreign), contents);
    // Producer changes do not disturb reproducibility for the current writer.
    assert_eq!(fs::read(&first).unwrap(), fs::read(&output).unwrap());
}

#[test]
#[allow(
    clippy::float_cmp,
    reason = "Persistence and copy-on-write tests require exact preservation of stored values"
)]
fn independently_rounded_python_angstrom_conversion_is_accepted() {
    let fixture =
        include_str!("../../../../tests/data/persistence/portable-python-angstrom-v1.rustiq.hex");
    let bytes: Vec<u8> = fixture
        .trim()
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("python.rustiq");
    fs::write(&path, bytes).unwrap();
    let mut restored = RustiQData::open(&path).unwrap();
    let requested = restored.request().unwrap().geometry().atoms[1].position[0];
    let resolved = restored.calculation().unwrap().atoms().nth(1).unwrap().1[0];
    // Python Decimal divided the exact binary64 request by the decimal V1
    // constant, rounding only the final result. It differs by one ULP from
    // dividing by the rounded binary64 constant, so exact equality would fail.
    assert_eq!(requested, 0.74);
    assert_ne!(requested / 0.529_177_210_903, resolved);
    assert_eq!(
        (requested / 0.529_177_210_903)
            .to_bits()
            .abs_diff(resolved.to_bits()),
        1
    );
    let identity = restored.manifest.scientific_identity.clone();
    let rewritten = dir.path().join("rewritten.rustiq");
    restored.write(&rewritten).unwrap();
    assert_eq!(
        RustiQData::open(&rewritten)
            .unwrap()
            .manifest
            .scientific_identity,
        identity
    );
    let mut contents = members(&path);
    edit_json(&mut contents, REQUEST_PATH, |r| {
        r["atoms"][1]["position"][0] = (requested + 1e-10).into();
    });
    let request = &contents.iter().find(|(n, _)| n == REQUEST_PATH).unwrap().1;
    let (size, digest) = (request.len(), sha256(request).to_string());
    edit_json(&mut contents, "manifest.json", |m| {
        m["calculations"][0]["request"]["size"] = size.into();
        m["calculations"][0]["request"]["digest"] = digest.into();
    });
    let invalid = dir.path().join("changed.rustiq");
    write_members(&invalid, &contents);
    assert!(matches!(
        RustiQData::open(&invalid),
        Err(PortableError::InvalidRequest(_))
    ));
}

#[test]
fn published_required_fields_match_v1_decoders() {
    use serde_json::{json, Value};
    let cases = [
        (
            include_str!("../../../../../../schemas/request-snapshot-v1.schema.json"),
            include_str!("../../../../tests/data/persistence/request-h2-v1.json"),
            true,
            vec![
                ("", ""),
                ("/hf", "hf"),
                ("/atoms/0", "atom"),
                ("/mp2", "mp2"),
            ],
        ),
        (
            include_str!("../../../../../../schemas/calculation-snapshot-v1.schema.json"),
            include_str!("../../../../tests/data/persistence/calculation-h2-v1.json"),
            false,
            vec![
                ("", ""),
                ("/hf", "hf"),
                ("/atoms/0", "atom"),
                ("/mp2", "mp2"),
                ("/basis/0", "ao"),
                ("/basis/0/components/0", "component"),
                ("/basis/0/components/0/primitives/0", "primitive"),
            ],
        ),
    ];
    for (schema, fixture, is_request, records) in cases {
        let schema: Value = serde_json::from_str(schema).unwrap();
        let mut value: Value = serde_json::from_str(fixture).unwrap();
        value["mp2"] = if is_request {
            json!({"frozen_orbitals":0,"memory_limit":{"kind":"auto"}})
        } else {
            json!({"frozen_orbitals":0})
        };
        for (path, definition) in records {
            let record_schema = if definition.is_empty() {
                &schema
            } else {
                &schema["$defs"][definition]
            };
            let required = record_schema["required"].as_array().unwrap();
            let properties = record_schema["properties"].as_object().unwrap();
            let decode = |candidate: Value| {
                if is_request {
                    serde_json::from_value::<RequestSnapshot>(candidate).is_ok()
                } else {
                    serde_json::from_value::<Snapshot>(candidate).is_ok()
                }
            };
            for field in properties.keys() {
                let mut candidate = value.clone();
                candidate
                    .pointer_mut(path)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove(field);
                assert_eq!(
                    decode(candidate),
                    !required.contains(&json!(field)),
                    "{is_request} {definition} {field}"
                );
            }
            let mut candidate = value.clone();
            candidate
                .pointer_mut(path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("unknown_v1_field".into(), json!(0));
            assert!(!decode(candidate), "unknown field in {definition}");
        }
    }
}

#[test]
#[allow(
    clippy::float_cmp,
    reason = "Persistence and copy-on-write tests require exact preservation of stored values"
)]
fn bundle_entries_have_independent_lazy_artifacts_and_shared_sources() {
    use crate::persistence::RustiQBundle;
    let basis = load_minimal_basis_file();
    let prepared = calculation();
    let changed = CalculationBuilder::new(prepared.get_molecule(), &basis)
        .with_integrals(crate::config::IntegralConfig {
            schwarz_threshold: None.into(),
        })
        .prepare()
        .unwrap();
    let mut first = data();
    first
        .add_source("batch.ncl", b"shared source".as_slice())
        .unwrap();
    let mut second = RustiQData::from_calculation(&changed).unwrap();
    second
        .add_source("batch.ncl", b"shared source".as_slice())
        .unwrap();
    second.set_eri(CompactEri::Zeroed(2)).unwrap();
    let mut bundle = RustiQBundle::new(vec![first, second]).unwrap();
    assert_eq!(bundle.sources().len(), 1);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("batch.rustiq");
    bundle.write(&path).unwrap();
    let contents = members(&path);
    let manifest: serde_json::Value = serde_json::from_slice(
        &contents
            .iter()
            .find(|(n, _)| n == "manifest.json")
            .unwrap()
            .1,
    )
    .unwrap();
    assert!(manifest.get("scientific_identity").is_none());
    assert_eq!(manifest["calculations"].as_array().unwrap().len(), 2);
    assert_eq!(
        contents
            .iter()
            .filter(|(n, _)| n.starts_with("sources/"))
            .count(),
        1
    );
    assert!(RustiQData::open(&path)
        .unwrap_err()
        .to_string()
        .contains("exactly one"));
    let mut restored = RustiQBundle::open(&path).unwrap();
    assert!(restored
        .calculations()
        .iter()
        .all(|data| data.ao_eri.is_none()));
    assert!(restored.calculations()[0].eri_is_compatible(&prepared));
    assert!(!restored.calculations()[1].eri_is_compatible(&prepared));
    assert!(restored.calculations()[1].eri_is_compatible(&changed));
    assert_eq!(
        restored.calculations_mut()[0]
            .read_eri()
            .unwrap()
            .ordered_values()[0],
        0.5
    );
    assert!(restored.calculations()[1].ao_eri.is_none());
    assert_eq!(
        restored.calculations_mut()[1]
            .read_eri()
            .unwrap()
            .ordered_values()[0],
        0.0
    );
    let copy = dir.path().join("copy.rustiq");
    restored.write(&copy).unwrap();
    assert_eq!(fs::read(&path).unwrap(), fs::read(&copy).unwrap());
}

#[test]
fn bundle_rejects_empty_duplicate_ids_cross_entry_and_conflicting_references() {
    use crate::persistence::RustiQBundle;
    assert!(RustiQBundle::new(vec![]).is_err());
    let dir = tempfile::tempdir().unwrap();
    let good = dir.path().join("good.rustiq");
    RustiQBundle::new(vec![data(), data()])
        .unwrap()
        .write(&good)
        .unwrap();
    let original = members(&good);
    for (field, value) in [
        ("/calculations", serde_json::json!([])),
        ("/calculations/1/id", serde_json::json!("calculation-0")),
        ("/calculations/1/id", serde_json::json!("../invalid")),
        (
            "/calculations/1/request/path",
            serde_json::json!(REQUEST_PATH),
        ),
        (
            "/calculations/0/artifacts/ao_eri/path",
            serde_json::json!(CALCULATION_PATH),
        ),
    ] {
        let mut contents = original.clone();
        edit_json(&mut contents, "manifest.json", |manifest| {
            *manifest.pointer_mut(field).unwrap() = value;
        });
        let bad = dir.path().join("bad.rustiq");
        write_members(&bad, &contents);
        assert!(RustiQBundle::open(&bad).is_err(), "{field}");
    }
    let mut contents = original;
    edit_json(&mut contents, "manifest.json", |manifest| {
        manifest["scientific_identity"] =
            manifest["calculations"][0]["scientific_identity"].clone();
    });
    let bad = dir.path().join("aggregate.rustiq");
    write_members(&bad, &contents);
    assert!(RustiQBundle::open(&bad).is_err());
}

#[test]
#[allow(
    clippy::float_cmp,
    reason = "Persistence and copy-on-write tests require exact preservation of stored values"
)]
fn corrupt_bundle_entry_does_not_prevent_independent_artifact_access_or_publish_partial_output() {
    use crate::persistence::RustiQBundle;
    let dir = tempfile::tempdir().unwrap();
    let good = dir.path().join("good.rustiq");
    RustiQBundle::new(vec![data(), data()])
        .unwrap()
        .write(&good)
        .unwrap();
    let mut contents = members(&good);
    *contents
        .iter_mut()
        .find(|(n, _)| n == AO_ERI_PATH)
        .unwrap()
        .1
        .last_mut()
        .unwrap() ^= 1;
    let bad = dir.path().join("bad.rustiq");
    write_members(&bad, &contents);
    let original = fs::read(&bad).unwrap();
    let mut bundle = RustiQBundle::open(&bad).unwrap();
    assert!(bundle.calculations_mut()[0].read_eri().is_err());
    assert_eq!(
        bundle.calculations_mut()[1]
            .read_eri()
            .unwrap()
            .ordered_values()[0],
        0.5
    );
    let output = dir.path().join("output.rustiq");
    assert!(bundle.write(&output).is_err());
    assert!(!output.exists());
    assert_eq!(fs::read(&bad).unwrap(), original);
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
}

#[test]
fn reads_python_multi_calculation_fixture() {
    let fixture =
        include_str!("../../../../tests/data/persistence/portable-python-multi-v1.rustiq.hex");
    let bytes: Vec<u8> = fixture
        .trim()
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("multi.rustiq");
    fs::write(&path, bytes).unwrap();
    let mut bundle = crate::persistence::RustiQBundle::open(&path).unwrap();
    assert_eq!(bundle.sources().len(), 1);
    assert_eq!(bundle.calculations().len(), 2);
    for data in bundle.calculations_mut() {
        assert!(data.ao_eri.is_none());
        assert_eq!(
            data.read_eri().unwrap().ordered_values(),
            &[0.5, 1.5, 2.5, 3.5, 4.5, 5.5]
        );
    }
}

#[test]
#[ignore = "writes a multi-calculation artifact for Python interoperability"]
fn writes_bundle_for_python_interoperability() {
    let path = std::env::var_os("RUSTIQ_ARCHIVE_TEST_OUTPUT").expect("output path");
    let mut bundle = crate::persistence::RustiQBundle::new(vec![data(), data()]).unwrap();
    bundle
        .add_source("batch.ncl", b"shared source".as_slice())
        .unwrap();
    bundle.write(path).unwrap();
}

#[test]
fn portable_manifest_golden_and_schema_required_fields_match_decoder() {
    use crate::persistence::manifest::PortableManifest;
    use serde_json::{json, Value};
    let value: Value = serde_json::from_str(include_str!(
        "../../../../tests/data/persistence/portable-manifest-h2-v1.json"
    ))
    .unwrap();
    let decoded: PortableManifest = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), value);
    let schema: Value = serde_json::from_str(include_str!(
        "../../../../../../schemas/portable-manifest-v1.schema.json"
    ))
    .unwrap();
    assert_eq!(schema["properties"]["calculations"]["minItems"], 1);
    for (path, definition) in [
        ("", ""),
        ("/calculations/0", "calculation"),
        ("/calculations/0/request", "snapshot"),
        ("/sources/0", "source"),
    ] {
        let record = if definition.is_empty() {
            &schema
        } else {
            &schema["$defs"][definition]
        };
        let required = record["required"].as_array().unwrap();
        for field in record["properties"].as_object().unwrap().keys() {
            let mut candidate = value.clone();
            candidate
                .pointer_mut(path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(field);
            assert_eq!(
                serde_json::from_value::<PortableManifest>(candidate).is_ok(),
                !required.contains(&json!(field)),
                "{definition} {field}"
            );
        }
        let mut candidate = value.clone();
        candidate
            .pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown_v1_field".into(), json!(0));
        assert!(serde_json::from_value::<PortableManifest>(candidate).is_err());
    }
}
