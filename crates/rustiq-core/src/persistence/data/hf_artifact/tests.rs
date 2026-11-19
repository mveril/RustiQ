use super::*;
use crate::{
    basis::BasisFile,
    calculation::CalculationBuilder,
    config::{HfConfig, HfMethod, MoleculeConfig},
    molecules::Geometry,
    persistence::{AoEriArtifact, RustiQBundle, Storage},
};

fn calculation(
    method: HfMethod,
    charge: i32,
    multiplicity: u8,
    redundant: bool,
) -> PreparedCalculation {
    let geometry = Geometry::from_source("h2.xyz", "2\nH2\nH 0 0 0\nH 1.4 0 0\n").unwrap();
    let mut json: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/sto-3g.json"
    )))
    .unwrap();
    if redundant {
        let shells = json["elements"]["1"]["electron_shells"]
            .as_array_mut()
            .unwrap();
        shells.push(shells[0].clone());
    }
    let basis = BasisFile::from_reader(serde_json::to_vec(&json).unwrap().as_slice()).unwrap();
    let hf = HfConfig {
        method: method.into(),
        ..HfConfig::default()
    };
    let molecule = MoleculeConfig {
        charge: charge.into(),
        multiplicity: std::num::NonZeroU8::new(multiplicity).unwrap().into(),
        ..MoleculeConfig::default()
    };
    CalculationBuilder::new(&geometry, &basis)
        .with_hf(hf)
        .with_molecule_config(molecule)
        .prepare()
        .unwrap()
}
fn fixture(method: HfMethod, redundant: bool) -> (PreparedCalculation, AnyHfSolution, RustiQData) {
    let prepared = calculation(method, 0, 1, redundant);
    let solution = prepared.run_hf().unwrap().into_converged().unwrap();
    let mut data = RustiQData::from_calculation(&prepared).unwrap();
    data.set::<ConvergedHfArtifact>(&ConvergedHf::from_solution(&solution).unwrap())
        .unwrap();
    (prepared, solution, data)
}
fn json(hf: impl std::borrow::Borrow<ConvergedHf>) -> serde_json::Value {
    serde_json::to_value(&*hf.borrow().0).unwrap()
}
fn from_json(json: serde_json::Value) -> ConvergedHf {
    ConvergedHf(Arc::new(serde_json::from_value(json).unwrap()))
}

#[test]
fn rhf_uhf_and_reduced_rank_roundtrip_without_integrals() {
    for method in [HfMethod::Rhf, HfMethod::Uhf] {
        for redundant in [false, true] {
            let (_, solution, mut data) = fixture(method, redundant);
            let before = json(data.get::<ConvergedHfArtifact>().unwrap().unwrap());
            match (
                data.get::<ConvergedHfArtifact>()
                    .unwrap()
                    .unwrap()
                    .orbitals(),
                &solution,
            ) {
                (HfOrbitals::Rhf(RhfOrbitals { shared }), AnyHfSolution::Restricted(hf)) => {
                    assert_eq!(shared.coefficients(), hf.mo_coefficients().as_slice());
                    assert_eq!(shared.energies(), hf.orbital_energies().as_slice());
                    assert_eq!(shared.occupied(), *hf.occupied_orbitals() as u64);
                }
                (
                    HfOrbitals::Uhf(UhfOrbitals {
                        channels: Spin { alpha, beta },
                    }),
                    AnyHfSolution::Unrestricted(hf),
                ) => {
                    assert_eq!(alpha.coefficients(), hf.mo_coefficients().alpha.as_slice());
                    assert_eq!(beta.coefficients(), hf.mo_coefficients().beta.as_slice());
                    assert_eq!(alpha.energies(), hf.orbital_energies().alpha.as_slice());
                    assert_eq!(beta.energies(), hf.orbital_energies().beta.as_slice());
                    assert_eq!(alpha.occupied(), hf.occupied_orbitals().alpha as u64);
                    assert_eq!(beta.occupied(), hf.occupied_orbitals().beta as u64);
                }
                _ => panic!("reference changed"),
            }

            assert_eq!(
                data.get::<ConvergedHfArtifact>()
                    .unwrap()
                    .unwrap()
                    .energies()[0],
                solution.summary().scf.electronic_energy
            );
            if redundant {
                assert!(
                    data.get::<ConvergedHfArtifact>()
                        .unwrap()
                        .unwrap()
                        .effective_rank()
                        < data
                            .get::<ConvergedHfArtifact>()
                            .unwrap()
                            .unwrap()
                            .basis_dimension()
                );
            }
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("hf.rustiq");
            data.write(&path).unwrap();
            let mut bundle = RustiQBundle::open(&path).unwrap();
            let restored = &mut bundle.calculations_mut()[0];
            assert!(restored.staged.is_empty());
            assert!(restored.check::<ConvergedHfArtifact>().unwrap());
            assert!(!restored.check::<AoEriArtifact>().unwrap());
            assert!(restored.get::<AoEriArtifact>().unwrap().is_none());
            assert_eq!(
                json(restored.get::<ConvergedHfArtifact>().unwrap().unwrap()),
                before
            );
            let first = restored.get::<ConvergedHfArtifact>().unwrap().unwrap();
            let second = restored.get::<ConvergedHfArtifact>().unwrap().unwrap();
            assert!(!Arc::ptr_eq(&first.0, &second.0));
            assert_eq!(json(first), json(second));
            let copy = dir.path().join("copy.rustiq");
            bundle.write(&copy).unwrap();
            let reread = RustiQData::open(copy).unwrap();
            assert_eq!(
                json(reread.get::<ConvergedHfArtifact>().unwrap().unwrap()),
                before
            );
        }
    }
}

#[test]
fn preserves_existing_integrals_and_old_archives() {
    let (_, solution, mut data) = fixture(HfMethod::Rhf, false);
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("old.rustiq");
    data.manifest.artifacts.remove(ConvergedHfArtifact::NAME);
    data.staged.remove(ConvergedHfArtifact::NAME);
    data.write_with_eri(&old, solution.ao_eri()).unwrap();
    let mut restored = RustiQData::open(&old).unwrap();
    assert!(restored.get::<ConvergedHfArtifact>().unwrap().is_none());
    restored
        .set::<ConvergedHfArtifact>(&ConvergedHf::from_solution(&solution).unwrap())
        .unwrap();
    let new = dir.path().join("new.rustiq");
    restored.write(&new).unwrap();
    let reopened = RustiQData::open(new).unwrap();
    assert!(reopened.get::<ConvergedHfArtifact>().unwrap().is_some());
    assert_eq!(
        reopened.read_eri().unwrap().ordered_values(),
        solution.ao_eri().ordered_values()
    );
}

#[test]
fn rejects_invalid_dimensions_occupations_numbers_versions_and_identity() {
    let (_, _, mut data) = fixture(HfMethod::Rhf, false);
    let valid = json(data.get::<ConvergedHfArtifact>().unwrap().unwrap());
    for (pointer, replacement) in [
        ("/basis_dimension", serde_json::json!(3)),
        ("/effective_rank", serde_json::json!(0)),
        ("/effective_rank", serde_json::json!(3)),
        ("/version", serde_json::json!(2)),
        ("/iterations", serde_json::json!(0)),
        ("/orbitals/shared/occupied", serde_json::json!(0)),
        ("/orbitals/shared/occupied", serde_json::json!(3)),
        ("/orbitals/shared/energies", serde_json::json!([0.0])),
        ("/orbitals/shared/coefficients", serde_json::json!([0.0])),
        ("/residual_norm", serde_json::json!(-1.0)),
        ("/delta_energy", serde_json::json!(1.0)),
    ] {
        let mut bad = valid.clone();
        *bad.pointer_mut(pointer).unwrap() = replacement;
        assert!(
            data.set::<ConvergedHfArtifact>(&from_json(bad)).is_err(),
            "{pointer}"
        );
    }
    for field in [
        "electronic_energy",
        "nuclear_repulsion_energy",
        "total_energy",
        "delta_energy",
        "residual_norm",
        "linear_dependency_threshold",
    ] {
        let mut record: HfRecord = serde_json::from_value(valid.clone()).unwrap();
        match field {
            "electronic_energy" => record.electronic_energy = f64::NAN,
            "nuclear_repulsion_energy" => record.nuclear_repulsion_energy = f64::INFINITY,
            "total_energy" => record.total_energy = f64::NEG_INFINITY,
            "delta_energy" => record.delta_energy = f64::NAN,
            "residual_norm" => record.residual_norm = f64::NAN,
            _ => record.linear_dependency_threshold = f64::NAN,
        }
        assert!(data
            .set::<ConvergedHfArtifact>(&ConvergedHf(Arc::new(record)))
            .is_err());
    }
    for charge_mult in [(1, 2), (0, 3)] {
        let prepared = calculation(HfMethod::Uhf, charge_mult.0, charge_mult.1, false);
        let mut other = RustiQData::from_calculation(&prepared).unwrap();
        assert!(other
            .set::<ConvergedHfArtifact>(&from_json(valid.clone()))
            .is_err());
    }
    let (_, _, other) = fixture(HfMethod::Uhf, false);
    assert!(data
        .set::<ConvergedHfArtifact>(&other.get::<ConvergedHfArtifact>().unwrap().unwrap())
        .is_err());
}

#[test]
fn rejects_corrupt_truncated_unknown_and_semantically_invalid_payloads() {
    let (_, _, data) = fixture(HfMethod::Rhf, false);
    let dir = tempfile::tempdir().unwrap();
    data.write_to(Storage::folder(dir.path())).unwrap();
    let original = std::fs::read(dir.path().join(HF_PATH)).unwrap();
    let manifest: crate::persistence::Manifest =
        serde_json::from_slice(&std::fs::read(dir.path().join("manifest.json")).unwrap()).unwrap();
    for mode in 0..5 {
        let mut m = manifest.clone();
        let mut bytes = original.clone();
        match mode {
            0 => {
                bytes.pop();
            }
            1 => bytes[0] = b' ',
            2 => {
                m.artifacts
                    .get_mut(ConvergedHfArtifact::NAME)
                    .unwrap()
                    .representation = "future-hf-v9".into();
            }
            3 | 4 => {
                let mut record: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                record[if mode == 3 {
                    "version"
                } else {
                    "effective_rank"
                }] = serde_json::json!(99);
                bytes = serde_json::to_vec(&record).unwrap();
                let a = m.artifacts.get_mut(ConvergedHfArtifact::NAME).unwrap();
                a.size = bytes.len() as u64;
                a.digest = crate::persistence::sha256(&bytes);
            }
            _ => unreachable!(),
        }
        std::fs::write(dir.path().join(HF_PATH), bytes).unwrap();
        let mut restored = RustiQData::read_manifest(Storage::folder(dir.path()), m).unwrap();
        restored.context = data.context.clone();
        assert!(
            restored.get::<ConvergedHfArtifact>().is_err(),
            "mode {mode}"
        );
    }
}

#[test]
fn open_shell_occupations_match_solution_and_context() {
    for (charge, multiplicity) in [(1, 2), (0, 3)] {
        let prepared = calculation(HfMethod::Uhf, charge, multiplicity, false);
        let hf = prepared.run_hf().unwrap().into_converged().unwrap();
        let solution = hf.as_unrestricted().unwrap();
        let mut data = RustiQData::from_calculation(&prepared).unwrap();
        let value = ConvergedHf::from_solution(&hf).unwrap();
        match value.orbitals() {
            HfOrbitals::Uhf(UhfOrbitals {
                channels: Spin { alpha, beta },
            }) => {
                assert_eq!(
                    alpha.coefficients(),
                    solution.mo_coefficients().alpha.as_slice()
                );
                assert_eq!(
                    beta.coefficients(),
                    solution.mo_coefficients().beta.as_slice()
                );
                assert_eq!(
                    alpha.energies(),
                    solution.orbital_energies().alpha.as_slice()
                );
                assert_eq!(beta.energies(), solution.orbital_energies().beta.as_slice());
                assert_eq!(alpha.occupied(), solution.occupied_orbitals().alpha as u64);
                assert_eq!(beta.occupied(), solution.occupied_orbitals().beta as u64);
                assert_ne!(alpha.occupied(), beta.occupied());
            }
            HfOrbitals::Rhf(RhfOrbitals { .. }) => panic!("expected UHF"),
        }
        data.set::<ConvergedHfArtifact>(&value).unwrap();
        assert!(data.hf_is_compatible(&prepared));
        let other = calculation(HfMethod::Uhf, 0, 1, false);
        assert!(data.matches_eri_identity(&other));
        assert!(!data.hf_is_compatible(&other));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("open-shell.rustiq");
        data.write(&path).unwrap();
        let restored = RustiQData::open(path).unwrap();
        assert_eq!(
            json(restored.get::<ConvergedHfArtifact>().unwrap().unwrap()),
            json(data.get::<ConvergedHfArtifact>().unwrap().unwrap())
        );
    }
}

#[test]
fn rejects_nonfinite_orbitals_and_unknown_layouts() {
    let (_, _, mut data) = fixture(HfMethod::Rhf, false);
    let valid = json(data.get::<ConvergedHfArtifact>().unwrap().unwrap());
    for coefficients in [true, false] {
        let mut record: HfRecord = serde_json::from_value(valid.clone()).unwrap();
        let HfOrbitals::Rhf(RhfOrbitals { shared }) = &mut record.orbitals else {
            unreachable!()
        };
        if coefficients {
            shared.coefficients[0] = f64::NAN;
        } else {
            shared.energies[0] = f64::INFINITY;
        }
        assert!(data
            .set::<ConvergedHfArtifact>(&ConvergedHf(Arc::new(record)))
            .is_err());
    }
    let mut bad = valid;
    bad["orbitals"]["reference"] = serde_json::json!("unknown");
    assert!(serde_json::from_value::<HfRecord>(bad).is_err());
}

#[test]
fn hf_identity_includes_settings_but_excludes_mp2() {
    let (prepared, _, data) = fixture(HfMethod::Rhf, false);
    let context = data.context.as_ref().unwrap();
    let original = context.0.hf_identity().unwrap();
    let snapshot = serde_json::to_value(&context.0).unwrap();
    for (pointer, value) in [
        ("/hf/max_iterations", serde_json::json!(200)),
        ("/hf/convergence_threshold", serde_json::json!(1e-7)),
        ("/hf/linear_dependency_threshold", serde_json::json!(1e-7)),
        ("/hf/eri_schwarz_threshold", serde_json::json!(1e-7)),
        ("/hf/diis_size", serde_json::json!(10)),
        ("/hf/guess", serde_json::json!({"kind": "zero"})),
        ("/charge", serde_json::json!(-2)),
        ("/multiplicity", serde_json::json!(3)),
    ] {
        let mut changed = snapshot.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        let changed: crate::persistence::calculation::Snapshot =
            serde_json::from_value(changed).unwrap();
        assert_ne!(original, changed.hf_identity().unwrap(), "{pointer}");
    }
    let mut changed = snapshot;
    changed["mp2"] = serde_json::json!({"frozen_orbitals": 0});
    let changed: crate::persistence::calculation::Snapshot =
        serde_json::from_value(changed).unwrap();
    assert_eq!(original, changed.hf_identity().unwrap());
    assert!(data.hf_is_compatible(&prepared));
}

#[test]
fn identity_survives_generic_solution_conversions() {
    let (_, solution, mut data) = fixture(HfMethod::Rhf, false);
    let hf = solution.into_restricted().unwrap();
    let generic = hf.into_scf();
    let artifact = generic.to_artifact().unwrap();
    data.set::<ConvergedHfArtifact>(&artifact).unwrap();
    let restored: AnyHfSolution = generic.into();
    data.set::<ConvergedHfArtifact>(&ConvergedHf::from_solution(&restored).unwrap())
        .unwrap();
}

#[test]
fn npy_leaves_have_numpy_shapes_and_corruption_is_rejected() {
    for method in [HfMethod::Rhf, HfMethod::Uhf] {
        let (_, _, data) = fixture(method, false);
        let dir = tempfile::tempdir().unwrap();
        data.write_to(Storage::folder(dir.path())).unwrap();
        let manifest: crate::persistence::Manifest =
            serde_json::from_slice(&std::fs::read(dir.path().join("manifest.json")).unwrap())
                .unwrap();
        let stems: &[&str] = if method == HfMethod::Rhf {
            &["orbitals"]
        } else {
            &["alpha", "beta"]
        };
        for stem in stems {
            for name in ["coefficients", "energies"] {
                let path = dir.path().join(if *stem == "orbitals" {
                    format!("orbitals/{name}.npy")
                } else {
                    format!("orbitals/{name}/{stem}.npy")
                });
                let original = std::fs::read(&path).unwrap();
                let npy = npyz::NpyFile::new(original.as_slice()).unwrap();
                assert_eq!(
                    npy.shape().len(),
                    if name == "coefficients" { 2 } else { 1 }
                );
                let mut bytes = original.clone();
                *bytes.last_mut().unwrap() ^= 1;
                std::fs::write(&path, bytes).unwrap();
                let mut restored =
                    RustiQData::read_manifest(Storage::folder(dir.path()), manifest.clone())
                        .unwrap();
                restored.context = data.context.clone();
                assert!(restored.get::<ConvergedHfArtifact>().is_err());
                assert!(restored.check::<ConvergedHfArtifact>().is_err());
                std::fs::write(&path, original).unwrap();
            }
        }
        let mut restored =
            RustiQData::read_manifest(Storage::folder(dir.path()), manifest).unwrap();
        restored.context = data.context.clone();
        let destination = tempfile::tempdir().unwrap();
        restored
            .write_to(Storage::folder(destination.path()))
            .unwrap();
        for stem in stems {
            assert!(destination
                .path()
                .join(if *stem == "orbitals" {
                    "orbitals/coefficients.npy".to_string()
                } else {
                    format!("orbitals/coefficients/{stem}.npy")
                })
                .is_file());
        }
    }
}

#[test]
fn staged_hf_clones_replace_independently_and_reread_payloads() {
    let (_, _, mut data) = fixture(HfMethod::Rhf, false);
    let clone = data.clone();
    let original = clone.get::<ConvergedHfArtifact>().unwrap().unwrap();
    let mut record: HfRecord = serde_json::from_value(json(&original)).unwrap();
    record.iterations += 1;
    let replacement = ConvergedHf(Arc::new(record));
    data.set::<ConvergedHfArtifact>(&replacement).unwrap();
    assert_eq!(
        data.get::<ConvergedHfArtifact>()
            .unwrap()
            .unwrap()
            .iterations(),
        replacement.iterations()
    );
    assert_eq!(
        clone
            .get::<ConvergedHfArtifact>()
            .unwrap()
            .unwrap()
            .iterations(),
        original.iterations()
    );
    let staged = clone.staged.get(ConvergedHfArtifact::NAME).unwrap();
    std::fs::write(staged.0.path().join("orbitals/energies.npy"), b"corrupt").unwrap();
    assert!(clone.get::<ConvergedHfArtifact>().is_err());
    assert!(data.get::<ConvergedHfArtifact>().is_ok());
    assert_eq!(original.basis_dimension(), replacement.basis_dimension());
}

#[test]
fn borrowed_hf_publication_does_not_stage_and_can_replace_corrupt_payload() {
    let directory = tempfile::tempdir().unwrap();
    let (prepared, _, data) = fixture(HfMethod::Uhf, false);
    let value = data.get::<ConvergedHfArtifact>().unwrap().unwrap();
    let entry = RustiQData::from_calculation(&prepared).unwrap();
    let path = directory.path().join("borrowed.rustiq");
    entry
        .write_with_artifacts(
            &path,
            &[crate::persistence::ArtifactRef::new::<ConvergedHfArtifact>(
                &value,
            )],
        )
        .unwrap();
    assert!(entry.staged.is_empty());
    assert!(!ConvergedHfArtifact::is_present(&entry));
    let reopened = RustiQData::open(path).unwrap();
    assert_eq!(
        json(reopened.get::<ConvergedHfArtifact>().unwrap().unwrap()),
        json(&value)
    );
    let staged = data.staged.get(ConvergedHfArtifact::NAME).unwrap();
    std::fs::write(
        staged.0.path().join("orbitals/energies/alpha.npy"),
        b"corrupt",
    )
    .unwrap();
    let destination = directory.path().join("corrupt.rustiq");
    let mut copy = data.clone();
    assert!(copy.write(&destination).is_err());
    assert!(!destination.exists());
    copy.write_with_artifacts(
        &destination,
        &[crate::persistence::ArtifactRef::new::<ConvergedHfArtifact>(
            &value,
        )],
    )
    .unwrap();
    assert!(RustiQData::open(destination)
        .unwrap()
        .get::<ConvergedHfArtifact>()
        .unwrap()
        .is_some());
}

#[test]
fn unsupported_npy_representation_label_is_rejected() {
    let (prepared, _, mut data) = fixture(HfMethod::Uhf, false);
    let entry = data
        .manifest
        .artifacts
        .get_mut(ConvergedHfArtifact::NAME)
        .unwrap();
    assert_eq!(entry.representation, HF_REPRESENTATION);
    entry.representation = "rustiq-converged-hf-npy-v1".into();
    assert!(!data.hf_is_compatible(&prepared));
    assert!(matches!(
        data.get::<ConvergedHfArtifact>(),
        Err(ArtifactError::UnsupportedRepresentation(_))
    ));
}

#[test]
fn typed_orbital_variants_preserve_descriptor_layout() {
    let rhf = serde_json::json!({"reference": "rhf", "shared": 7});
    let uhf = serde_json::json!({"reference": "uhf", "alpha": 7, "beta": 3});
    for value in [rhf, uhf] {
        let orbitals: HfOrbitals<u64> = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(orbitals).unwrap(), value);
    }
    for invalid in [
        serde_json::json!({"reference": "rhf", "alpha": 7, "beta": 3}),
        serde_json::json!({"reference": "uhf", "shared": 7}),
        serde_json::json!({"reference": "rhf", "shared": 7, "beta": 3}),
        serde_json::json!({"reference": "rohf", "shared": 7}),
    ] {
        assert!(serde_json::from_value::<HfOrbitals<u64>>(invalid).is_err());
    }
}

fn mp2_energy(solution: &AnyHfSolution) -> f64 {
    use crate::config::Mp2Config;
    match solution {
        AnyHfSolution::Restricted(hf) => hf.mp2(Mp2Config::default()).unwrap().correlation_energy,
        AnyHfSolution::Unrestricted(hf) => hf.mp2(Mp2Config::default()).unwrap().correlation_energy,
    }
}

#[test]
fn restore_hf_roundtrip_retains_mp2_and_summary() {
    use crate::{
        calculation::{RhfSolution, UhfSolution},
        persistence::RestoreFromRustiQ,
    };
    for (method, charge, multiplicity) in [
        (HfMethod::Rhf, 0, 1),
        (HfMethod::Uhf, 0, 1),
        (HfMethod::Uhf, 1, 2),
    ] {
        for redundant in [false, true] {
            let prepared = calculation(method, charge, multiplicity, redundant);
            let solution = prepared.run_hf().unwrap().into_converged().unwrap();
            let mut data = RustiQData::from_calculation(&prepared).unwrap();
            data.set::<ConvergedHfArtifact>(&ConvergedHf::from_solution(&solution).unwrap())
                .unwrap();
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("hf.rustiq");
            data.write_with_eri(&path, solution.ao_eri()).unwrap();
            let bytes = std::fs::read(&path).unwrap();
            let data = RustiQData::open(&path).unwrap();
            let restored: AnyHfSolution = data.restore().unwrap();
            assert!((mp2_energy(&restored) - mp2_energy(&solution)).abs() < 1e-12);
            let original = &solution.summary().scf;
            let summary = &restored.summary().scf;
            assert_eq!(
                restored.summary().persistence_identity,
                solution.summary().persistence_identity
            );
            assert_eq!(summary.total_energy, original.total_energy);
            assert_eq!(summary.iterations, original.iterations);
            assert_eq!(summary.residual_norm, original.residual_norm);
            assert_eq!(summary.orthogonalization, original.orthogonalization);
            assert_eq!(summary.spin, original.spin);
            assert!(
                (summary.energy_details.kinetic_energy - original.energy_details.kinetic_energy)
                    .abs()
                    < 1e-12
            );
            assert!(
                (summary.energy_details.nuclear_attraction_energy
                    - original.energy_details.nuclear_attraction_energy)
                    .abs()
                    < 1e-12
            );
            assert!(
                (summary.energy_details.electron_repulsion_energy
                    - original.energy_details.electron_repulsion_energy)
                    .abs()
                    < 1e-12
            );
            assert_eq!(summary.timings.total, std::time::Duration::ZERO);
            assert_eq!(
                restored.ao_eri().ordered_values(),
                solution.ao_eri().ordered_values()
            );
            match &solution {
                AnyHfSolution::Restricted(hf) => {
                    let typed = RhfSolution::restore_from_rustiq(&data).unwrap();
                    assert_eq!(typed.mo_coefficients(), hf.mo_coefficients());
                    assert_eq!(typed.orbital_energies(), hf.orbital_energies());
                    assert!(matches!(
                        data.restore::<UhfSolution>(),
                        Err(crate::persistence::PortableError::HfReferenceMismatch { .. })
                    ));
                }
                AnyHfSolution::Unrestricted(hf) => {
                    let typed: UhfSolution = data.restore().unwrap();
                    assert_eq!(typed.mo_coefficients(), hf.mo_coefficients());
                    assert_eq!(typed.orbital_energies(), hf.orbital_energies());
                    assert!(matches!(
                        RhfSolution::restore_from_rustiq(&data),
                        Err(crate::persistence::PortableError::HfReferenceMismatch { .. })
                    ));
                }
            }
            drop(data);
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            drop(solution);
            assert!(mp2_energy(&restored).is_finite());
        }
    }
}

#[test]
fn restore_hf_requires_both_artifacts_and_valid_payloads() {
    use crate::persistence::PortableError;
    let (_, solution, mut data) = fixture(HfMethod::Rhf, false);
    assert!(matches!(
        data.restore::<AnyHfSolution>(),
        Err(PortableError::MissingArtifact("ao_eri"))
    ));
    data.set::<AoEriArtifact>(solution.ao_eri()).unwrap();
    let hf = data
        .manifest
        .artifacts
        .remove(ConvergedHfArtifact::NAME)
        .unwrap();
    assert!(matches!(
        data.restore::<AnyHfSolution>(),
        Err(PortableError::MissingArtifact("converged_hf"))
    ));
    data.manifest
        .artifacts
        .insert(ConvergedHfArtifact::NAME.into(), hf);
    let eri = data
        .manifest
        .artifacts
        .get_mut(AoEriArtifact::NAME)
        .unwrap();
    eri.digest = crate::persistence::sha256(b"corrupt");
    assert!(data.restore::<AnyHfSolution>().is_err());
    data.set::<AoEriArtifact>(solution.ao_eri()).unwrap();
    let mut other = RustiQData::from_calculation(&calculation(HfMethod::Uhf, 1, 2, false)).unwrap();
    data.context = other.context.take();
    assert!(data.restore::<AnyHfSolution>().is_err());
}

#[test]
fn restore_hf_open_shell_oh_retains_nonzero_mp2() {
    use crate::{calculation::UhfSolution, config::Mp2Config, persistence::RestoreFromRustiQ};
    let geometry = crate::test_utils::load_sample_geometry_in_bohr("samples/oh/oh.xyz");
    let basis = crate::test_utils::load_minimal_basis_file();
    let molecule = MoleculeConfig {
        multiplicity: std::num::NonZeroU8::new(2).unwrap().into(),
        ..MoleculeConfig::default()
    };
    let prepared = CalculationBuilder::new(&geometry, &basis)
        .with_molecule_config(molecule)
        .prepare()
        .unwrap();
    let solution = prepared.run_hf().unwrap().into_converged().unwrap();
    let original = solution.as_unrestricted().unwrap();
    assert_ne!(
        original.occupied_orbitals().alpha,
        original.occupied_orbitals().beta
    );
    assert!(original.occupied_orbitals().beta > 0);
    let data = RustiQData::from_calculation(&prepared).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("oh.rustiq");
    data.write_with_artifacts(
        &path,
        &[
            crate::persistence::ArtifactRef::new::<ConvergedHfArtifact>(
                &ConvergedHf::from_solution(&solution).unwrap(),
            ),
            crate::persistence::ArtifactRef::new::<AoEriArtifact>(solution.ao_eri()),
        ],
    )
    .unwrap();
    let data = RustiQData::open(path).unwrap();
    let restored = UhfSolution::restore_from_rustiq(&data).unwrap();
    let expected = original
        .mp2(Mp2Config::default())
        .unwrap()
        .correlation_energy;
    assert!(expected.abs() > 1e-6);
    assert!(
        (restored
            .mp2(Mp2Config::default())
            .unwrap()
            .correlation_energy
            - expected)
            .abs()
            < 1e-12
    );
    assert_eq!(restored.mo_coefficients(), original.mo_coefficients());
    assert_eq!(restored.orbital_energies(), original.orbital_energies());
    assert_eq!(
        restored.summary().scf.total_energy,
        original.summary().scf.total_energy
    );
    let components = &restored.summary().scf.energy_details;
    let total = components.kinetic_energy
        + components.nuclear_attraction_energy
        + components.electron_repulsion_energy;
    assert!((total - restored.summary().scf.electronic_energy).abs() < 1e-12);
    assert_eq!(restored.summary().scf.spin, original.summary().scf.spin);
}

#[test]
fn restore_hf_rejects_corrupt_or_unsupported_hf_and_incompatible_eri() {
    use crate::persistence::PortableError;
    let (_, solution, mut data) = fixture(HfMethod::Rhf, false);
    data.set::<AoEriArtifact>(solution.ao_eri()).unwrap();
    let original = data.manifest.artifacts[ConvergedHfArtifact::NAME].clone();
    data.manifest
        .artifacts
        .get_mut(ConvergedHfArtifact::NAME)
        .unwrap()
        .digest = crate::persistence::sha256(b"corrupt HF");
    assert!(matches!(
        data.restore::<AnyHfSolution>(),
        Err(PortableError::Artifact(ArtifactError::IntegrityMismatch(_)))
    ));
    data.manifest
        .artifacts
        .insert(ConvergedHfArtifact::NAME.into(), original.clone());
    data.manifest
        .artifacts
        .get_mut(ConvergedHfArtifact::NAME)
        .unwrap()
        .representation = "unsupported-hf".into();
    assert!(matches!(
        data.restore::<AnyHfSolution>(),
        Err(PortableError::Artifact(
            ArtifactError::UnsupportedRepresentation(_)
        ))
    ));
    data.manifest
        .artifacts
        .insert(ConvergedHfArtifact::NAME.into(), original);
    data.manifest.scientific_identity.digest = crate::persistence::sha256(b"other ERI context");
    assert!(matches!(
        data.restore::<AnyHfSolution>(),
        Err(PortableError::IncompatibleEri)
    ));
    data.context = None;
    assert!(matches!(
        data.restore::<AnyHfSolution>(),
        Err(PortableError::InvalidCalculation(_))
    ));
}
