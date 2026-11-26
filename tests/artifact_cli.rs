#![allow(
    clippy::unwrap_used,
    reason = "Integration tests fail immediately on unexpected fixture errors"
)]

use rustiq_core::persistence::{
    AoEriArtifact, ConvergedHfArtifact, ExportError, ExportFormat, HfOrbitals, RustiQBundle,
    RustiQData,
};
use serde_json::{json, Value};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use tempfile::TempDir;

fn fixture() -> TempDir {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let store = root.join("data/rustiq/basis_sets");
    fs::create_dir_all(&store).unwrap();
    fs::write(
        store.join("sto-3g.json"),
        include_bytes!("data/sto-3g.json"),
    )
    .unwrap();
    fs::write(root.join("molecule.xyz"), "2\nH2\nH 0 0 0\nH 0 0 0.74\n").unwrap();
    fs::write(root.join("changed.xyz"), "2\nH2\nH 0 0 0\nH 0 0 0.80\n").unwrap();
    fs::write(
        root.join("run.toml"),
        "[molecule]\ngeometry = 'molecule.xyz'\n[basis]\nname = 'sto-3g'\n",
    )
    .unwrap();
    directory
}

fn command(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rustiq"))
        .current_dir(root)
        .env("RUSTIQ_DATA_HOME", root.join("data"))
        .env("RUSTIQ_CACHE_HOME", root.join("cache"))
        .env("RUSTIQ_AUTO_DOWNLOAD", "0")
        .args(["--color", "never"])
        .args(args)
        .output()
        .unwrap()
}

fn success(root: &Path, args: &[&str]) -> Value {
    let output = command(root, args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn calculate(root: &Path, args: &[&str]) -> Value {
    let mut all = vec!["run", "run.toml", "--format", "json"];
    all.extend_from_slice(args);
    let result = success(root, &all);
    let schema: Value =
        serde_json::from_str(include_str!("../schemas/calculation-output-v1.schema.json")).unwrap();
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(&result)
        .unwrap();
    result
}

fn report(value: &Value) -> &Value {
    &value["artifacts"][0]
}

#[test]
fn creates_inspects_and_reuses_read_only_sources_without_original_inputs() {
    let directory = fixture();
    let root = directory.path();
    let fresh = calculate(root, &["--artifact", "source.rustiq"]);
    assert_eq!(report(&fresh)["origin"], "computation");
    let source = root.join("source.rustiq");
    let bytes = fs::read(&source).unwrap();
    let mut permissions = fs::metadata(&source).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&source, permissions).unwrap();
    let reused = calculate(
        root,
        &["--reuse", "source.rustiq", "--artifact", "result.rustiq"],
    );
    assert_eq!(
        report(&reused),
        &json!({"name":"ao_eri", "decision":"reused", "origin":"archive", "source_index":0})
    );
    assert_eq!(fresh["calculation"], reused["calculation"]);
    assert_eq!(fs::read(&source).unwrap(), bytes);
    fs::remove_file(root.join("run.toml")).unwrap();
    fs::remove_file(root.join("molecule.xyz")).unwrap();
    let inspected = success(
        root,
        &["artifact", "inspect", "source.rustiq", "--format", "json"],
    );
    let schema: Value = serde_json::from_str(include_str!(
        "../schemas/artifact-inspection-v1.schema.json"
    ))
    .unwrap();
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(&inspected)
        .unwrap();
    assert_eq!(inspected["payloads_verified"], false);
    assert_eq!(inspected["sources"].as_array().unwrap().len(), 2);
    let entry = &inspected["calculations"][0];
    assert!(entry["requested"]["configuration"]
        .as_str()
        .unwrap()
        .contains("Auto"));
    assert!(entry["resolved"]["configuration"]
        .as_str()
        .unwrap()
        .contains("Rhf"));
    assert!(entry["requested"]["configuration"]
        .as_str()
        .unwrap()
        .contains("Angstrom"));
    assert!(entry["resolved"]["configuration"]
        .as_str()
        .unwrap()
        .contains("Bohr"));
    assert!(!entry["requested"]["configuration"]
        .as_str()
        .unwrap()
        .contains("[cache]"));
    let text = command(root, &["artifact", "inspect", "result.rustiq"]);
    assert!(text.status.success());
    let text = String::from_utf8_lossy(&text.stdout);
    assert!(text.contains("Requested calculation") && text.contains("Resolved calculation"));
    assert!(text.contains("ao_eri"));
}

#[test]
fn explicit_archive_precedes_cache_and_missing_or_incompatible_fall_back() {
    let directory = fixture();
    let root = directory.path();
    fs::write(
        root.join("run.toml"),
        "[basis]\nname = 'sto-3g'\n[cache]\nenabled = true\n",
    )
    .unwrap();
    let first = calculate(
        root,
        &["--artifact", "source.rustiq", "--cache-dir", "eri-cache"],
    );
    assert_eq!(report(&first)["origin"], "computation");
    let mut empty = RustiQData::open(root.join("source.rustiq")).unwrap();
    let prepared = empty.prepare_calculation().unwrap();
    empty = RustiQData::from_calculation(&prepared).unwrap();
    empty.write(root.join("empty.rustiq")).unwrap();
    let cached = calculate(
        root,
        &["--reuse", "empty.rustiq", "--cache-dir", "eri-cache"],
    );
    assert_eq!(report(&cached)["decision"], "missing");
    assert_eq!(report(&cached)["origin"], "cache");
    let archive = calculate(
        root,
        &["--reuse", "source.rustiq", "--cache-dir", "unused-cache"],
    );
    assert_eq!(report(&archive)["origin"], "archive");
    assert!(!root.join("unused-cache").exists());
    fs::write(
        root.join("run.toml"),
        "[molecule]\ngeometry = 'changed.xyz'\n[basis]\nname = 'sto-3g'\n[cache]\nenabled = true\n",
    )
    .unwrap();
    let changed = calculate(
        root,
        &["--reuse", "source.rustiq", "--cache-dir", "eri-cache"],
    );
    assert_eq!(report(&changed)["decision"], "incompatible");
    assert_eq!(report(&changed)["origin"], "computation");
    let changed_again = calculate(
        root,
        &["--reuse", "source.rustiq", "--cache-dir", "eri-cache"],
    );
    assert_eq!(report(&changed_again)["origin"], "cache");
    fs::write(root.join("run.toml"), "[basis]\nname = 'sto-3g'\n").unwrap();
    let missing = calculate(root, &["--reuse", "empty.rustiq"]);
    assert_eq!(report(&missing)["origin"], "computation");
    assert_eq!(first["calculation"], missing["calculation"]);
}

#[test]
fn same_path_replacement_retains_new_request_and_failure_preserves_source() {
    let directory = fixture();
    let root = directory.path();
    calculate(root, &["--artifact", "state.rustiq"]);
    let bytes = fs::read(root.join("state.rustiq")).unwrap();
    fs::write(root.join("run.toml"), "[basis]\nname = 'missing-basis'\n").unwrap();
    let failed = command(
        root,
        &[
            "run",
            "run.toml",
            "--reuse",
            "state.rustiq",
            "--artifact",
            "state.rustiq",
            "--format",
            "json",
        ],
    );
    assert!(!failed.status.success());
    assert_eq!(fs::read(root.join("state.rustiq")).unwrap(), bytes);
    fs::write(
        root.join("run.toml"),
        "[basis]\nname = 'sto-3g'\n[method.hf]\nmethod = 'Rhf'\n",
    )
    .unwrap();
    let result = calculate(
        root,
        &["--reuse", "state.rustiq", "--artifact", "./state.rustiq"],
    );
    assert_eq!(report(&result)["origin"], "archive");
    assert_ne!(fs::read(root.join("state.rustiq")).unwrap(), bytes);
    let restored = RustiQData::open(root.join("state.rustiq")).unwrap();
    assert!(restored.get::<AoEriArtifact>().unwrap().is_some());
    assert!(restored
        .sources()
        .any(|source| source.bytes() == fs::read(root.join("run.toml")).unwrap()));
    let duplicate = command(root, &["run", "run.toml", "--artifact", "state.rustiq"]);
    assert!(!duplicate.status.success());
}

#[test]
fn forced_artifact_replacement_preserves_previous_bytes_on_failure() {
    let directory = fixture();
    let root = directory.path();
    calculate(root, &["--artifact", "result.rustiq"]);
    let path = root.join("result.rustiq");
    let original = fs::read(&path).unwrap();

    let refused = command(root, &["run", "run.toml", "--artifact", "result.rustiq"]);
    assert!(!refused.status.success());
    assert_eq!(fs::read(&path).unwrap(), original);

    fs::write(root.join("run.toml"), "[basis]\nname = 'missing-basis'\n").unwrap();
    let failed = command(
        root,
        &["run", "run.toml", "--artifact", "result.rustiq", "--force"],
    );
    assert!(!failed.status.success());
    assert_eq!(fs::read(&path).unwrap(), original);

    fs::write(
        root.join("run.toml"),
        "[molecule]\ngeometry = 'changed.xyz'\n[basis]\nname = 'sto-3g'\n",
    )
    .unwrap();
    calculate(root, &["--artifact", "result.rustiq", "--force"]);
    assert_ne!(fs::read(&path).unwrap(), original);
    assert!(RustiQBundle::open(&path).is_ok());
}

#[test]
fn corrupt_selected_payload_is_an_error_and_invalid_archive_fails_before_execution() {
    let directory = fixture();
    let root = directory.path();
    calculate(root, &["--artifact", "source.rustiq"]);
    let mut bytes = fs::read(root.join("source.rustiq")).unwrap();
    let offset = bytes
        .windows(6)
        .position(|value| value == b"\x93NUMPY")
        .unwrap();
    let header_length = usize::from(u16::from_le_bytes([bytes[offset + 8], bytes[offset + 9]]));
    bytes[offset + 10 + header_length] ^= 1;
    fs::write(root.join("source.rustiq"), &bytes).unwrap();
    let inspect = command(
        root,
        &["artifact", "inspect", "source.rustiq", "--format", "json"],
    );
    assert!(inspect.status.success());
    let failed = command(
        root,
        &[
            "run",
            "run.toml",
            "--reuse",
            "source.rustiq",
            "--artifact",
            "source.rustiq",
            "--format",
            "json",
        ],
    );
    assert!(!failed.status.success());
    assert_eq!(fs::read(root.join("source.rustiq")).unwrap(), bytes);
    fs::write(root.join("invalid.rustiq"), "invalid ZIP").unwrap();
    let failed = command(
        root,
        &[
            "run",
            "run.toml",
            "--reuse",
            "invalid.rustiq",
            "--artifact",
            "output.rustiq",
        ],
    );
    assert!(!failed.status.success());
    assert!(!String::from_utf8_lossy(&failed.stdout).contains("Preparing SCF"));
    assert!(!root.join("output.rustiq").exists());
}

fn validate_batch(value: &Value) {
    let mut schema: Value =
        serde_json::from_str(include_str!("../schemas/batch-output-v1.schema.json")).unwrap();
    let calculation: Value =
        serde_json::from_str(include_str!("../schemas/calculation-output-v1.schema.json")).unwrap();
    schema["$defs"] = json!({"calculation": calculation});
    schema["properties"]["calculations"]["items"]["oneOf"][0]["properties"]["result"]["$ref"] =
        "#/$defs/calculation".into();
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(value)
        .unwrap();
}

#[test]
fn nickel_matches_each_calculation_and_writes_only_the_current_collection() {
    let directory = fixture();
    let root = directory.path();
    fs::write(root.join("study.ncl"), "[{ basis.name = \"sto-3g\", molecule.geometry = \"changed.xyz\" }, { basis.name = \"sto-3g\" }]").unwrap();
    let initial = success(
        root,
        &[
            "run",
            "study.ncl",
            "--artifact",
            "study.rustiq",
            "--format",
            "json",
        ],
    );
    validate_batch(&initial);
    let source_bytes = fs::read(root.join("study.rustiq")).unwrap();
    let single = calculate(
        root,
        &["--reuse", "study.rustiq", "--artifact", "single.rustiq"],
    );
    assert_eq!(report(&single)["source_index"], 1);
    assert_eq!(
        RustiQBundle::open(root.join("single.rustiq"))
            .unwrap()
            .calculations()
            .len(),
        1
    );
    fs::write(root.join("study.ncl"), "[{ basis.name = \"sto-3g\" }, { basis.name = \"sto-3g\", molecule.geometry = \"changed.xyz\" }, { basis.name = \"sto-3g\" }]").unwrap();
    let reordered = success(
        root,
        &[
            "run",
            "study.ncl",
            "--reuse",
            "study.rustiq",
            "--artifact",
            "reordered.rustiq",
            "--format",
            "json",
        ],
    );
    validate_batch(&reordered);
    for (index, source_index) in [1, 0, 1].into_iter().enumerate() {
        let entry = &reordered["calculations"][index]["result"];
        assert_eq!(report(entry)["source_index"], source_index);
        assert_eq!(report(entry)["origin"], "archive");
    }
    let bundle = RustiQBundle::open(root.join("reordered.rustiq")).unwrap();
    assert_eq!(bundle.calculations().len(), 3);
    assert_eq!(bundle.sources().len(), 3);
    assert_eq!(fs::read(root.join("study.rustiq")).unwrap(), source_bytes);
    let reused_once = calculate(root, &["--reuse", "reordered.rustiq"]);
    assert_eq!(report(&reused_once)["source_index"], 0);
}

#[test]
fn publishes_converged_orbitals_for_rhf_and_uhf_mp2() {
    let directory = fixture();
    let root = directory.path();
    for (method, mp2) in [("Rhf", ""), ("Uhf", "[method.mp2]\n")] {
        let runfile = format!(
            "[molecule]\ngeometry = 'molecule.xyz'\n[basis]\nname = 'sto-3g'\n[method.hf]\nmethod = '{method}'\n{mp2}"
        );
        fs::write(root.join("run.toml"), runfile).unwrap();
        let path = format!("{method}.rustiq");
        let result = calculate(root, &["--artifact", &path]);
        assert_eq!(result["calculation"]["mp2"].is_object(), !mp2.is_empty());
        let data = RustiQData::open(root.join(&path)).unwrap();
        let hf = data.get::<ConvergedHfArtifact>().unwrap().unwrap();
        assert_eq!(matches!(hf.orbitals(), HfOrbitals::Rhf(_)), method == "Rhf");
        assert_eq!(
            hf.energies()[2].to_bits(),
            result["calculation"]["hf"]["total_energy"]
                .as_f64()
                .unwrap()
                .to_bits()
        );
        assert!(data.get::<AoEriArtifact>().unwrap().is_some());
        let verified = command(root, &["artifact", "verify", &path]);
        assert!(
            verified.status.success(),
            "{}",
            String::from_utf8_lossy(&verified.stderr)
        );
        assert!(String::from_utf8_lossy(&verified.stdout).contains("Verified:     2"));
    }
}

#[test]
fn non_converged_batch_publishes_eri_but_any_execution_failure_blocks_publication() {
    let directory = fixture();
    let root = directory.path();
    fs::write(
        root.join("study.ncl"),
        "[{ basis.name = \"sto-3g\", method.hf.max_iterations = 1 }, { basis.name = \"sto-3g\" }]",
    )
    .unwrap();
    let output = command(
        root,
        &[
            "run",
            "study.ncl",
            "--artifact",
            "state.rustiq",
            "--format",
            "json",
        ],
    );
    assert!(!output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate_batch(&value);
    assert_eq!(value["calculations"][0]["status"], "non_converged");
    let bundle = RustiQBundle::open(root.join("state.rustiq")).unwrap();
    assert_eq!(bundle.calculations().len(), 2);
    for (index, data) in bundle.calculations().iter().enumerate() {
        assert!(data.get::<AoEriArtifact>().unwrap().is_some());
        assert_eq!(
            data.get::<ConvergedHfArtifact>().unwrap().is_some(),
            index == 1
        );
    }
    drop(bundle);
    let bytes = fs::read(root.join("state.rustiq")).unwrap();
    for bad in [
        "[{ basis.name = \"sto-3g\" }, { basis.name = \"missing-basis\" }, { basis.name = \"sto-3g\" }]",
        "[{ basis.name = \"sto-3g\" }, { basis.name = \"sto-3g\", method.hf.max_iterations = 1, method.mp2 = {} }, { basis.name = \"sto-3g\" }]",
    ] {
        fs::write(root.join("study.ncl"), bad).unwrap();
        let output = command(root, &["run", "study.ncl", "--reuse", "state.rustiq", "--artifact", "state.rustiq", "--format", "json"]);
        assert!(!output.status.success());
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate_batch(&value);
        assert_eq!(value["calculations"][1]["status"], "error");
        assert_eq!(value["calculations"][2]["status"], "success");
        assert_eq!(fs::read(root.join("state.rustiq")).unwrap(), bytes);
        let output = command(root, &["run", "study.ncl", "--artifact", "failed.rustiq", "--format", "json"]);
        assert!(!output.status.success());
        assert!(!root.join("failed.rustiq").exists());
    }
}

#[test]
fn publication_error_keeps_complete_calculation_and_batch_json() {
    let directory = fixture();
    let root = directory.path();
    let output = command(
        root,
        &[
            "run",
            "run.toml",
            "--artifact",
            "absent/result.rustiq",
            "--format",
            "json",
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["calculation"]["hf"]["converged"], true);
    fs::write(
        root.join("study.ncl"),
        "[{ basis.name = \"sto-3g\" }, { basis.name = \"sto-3g\" }]",
    )
    .unwrap();
    let output = command(
        root,
        &[
            "run",
            "study.ncl",
            "--artifact",
            "absent/result.rustiq",
            "--format",
            "json",
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate_batch(&result);
    assert_eq!(result["calculations"].as_array().unwrap().len(), 2);
    assert!(result["calculations"]
        .as_array()
        .unwrap()
        .iter()
        .all(|entry| entry["status"] == "success"));
    assert!(!root.join("absent").exists());
}

#[cfg(unix)]
#[test]
fn symbolic_alias_replaces_the_target_and_preserves_the_alias() {
    let directory = fixture();
    let root = directory.path();
    calculate(root, &["--artifact", "state.rustiq"]);
    std::os::unix::fs::symlink("state.rustiq", root.join("alias.rustiq")).unwrap();
    let result = calculate(
        root,
        &["--reuse", "state.rustiq", "--artifact", "alias.rustiq"],
    );
    assert_eq!(report(&result)["origin"], "archive");
    assert!(fs::symlink_metadata(root.join("alias.rustiq"))
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(RustiQBundle::open(root.join("alias.rustiq")).is_ok());
}

#[test]
fn portable_reruns_preserve_convergence_exit_codes() {
    let success = "{ basis.name = \"sto-3g\" }";
    let unconverged = "{ basis.name = \"sto-3g\", method.hf.max_iterations = 1 }";
    for (entries, expected) in [
        (vec![unconverged], 3),
        (vec![unconverged, unconverged], 3),
        (vec![success, unconverged], 4),
    ] {
        let directory = fixture();
        let root = directory.path();
        fs::write(root.join("study.ncl"), format!("[{}]", entries.join(", "))).unwrap();
        let output = command(
            root,
            &[
                "run",
                "study.ncl",
                "--artifact",
                "status.rustiq",
                "--format",
                "json",
            ],
        );
        assert_eq!(
            output.status.code(),
            Some(expected),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let rerun = command(root, &["run", "status.rustiq", "--format", "json"]);
        assert_eq!(
            rerun.status.code(),
            Some(expected),
            "{}",
            String::from_utf8_lossy(&rerun.stderr)
        );
        let value: Value = serde_json::from_slice(&rerun.stdout).unwrap();
        if entries.len() > 1 {
            validate_batch(&value);
        } else {
            assert_eq!(value["calculation"]["hf"]["converged"], false);
        }
        if expected == 4 {
            let failed = command(
                root,
                &[
                    "run",
                    "study.ncl",
                    "--artifact",
                    "absent/status.rustiq",
                    "--format",
                    "json",
                ],
            );
            assert_eq!(failed.status.code(), Some(1));
            let _: Value = serde_json::from_slice(&failed.stdout).unwrap();
        }
    }
}

#[test]
fn reuse_without_publication_does_not_need_temporary_storage() {
    let directory = fixture();
    let root = directory.path();
    calculate(root, &["--artifact", "source.rustiq"]);
    let before = fs::read(root.join("source.rustiq")).unwrap();
    let blocked = root.join("unavailable-temporary-directory");
    fs::write(&blocked, b"not a directory").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rustiq"))
        .current_dir(root)
        .env("RUSTIQ_DATA_HOME", root.join("data"))
        .env("RUSTIQ_CACHE_HOME", root.join("cache"))
        .env("RUSTIQ_AUTO_DOWNLOAD", "0")
        .env("TMPDIR", &blocked)
        .env("TMP", &blocked)
        .env("TEMP", &blocked)
        .args([
            "run",
            "run.toml",
            "--reuse",
            "source.rustiq",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report(&result)["decision"], "reused");
    assert_eq!(report(&result)["origin"], "archive");
    assert_eq!(fs::read(root.join("source.rustiq")).unwrap(), before);
}

#[test]
fn verification_counts_current_artifacts() {
    let directory = fixture();
    let root = directory.path();
    calculate(root, &["--artifact", "verify.rustiq"]);
    let output = command(root, &["artifact", "verify", "verify.rustiq"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.starts_with("Verification successful"), "{text}");
    assert_eq!(output.stderr, b"");
    assert!(text.contains("Verified:     2"), "{text}");
    assert!(text.contains("Failed:       0"), "{text}");
    assert!(text.contains("Skipped:      0"), "{text}");
}

// Rewrite only the manifest or an artifact payload, leaving snapshot digests intact.
fn rewrite_verification_bundle(path: &Path, change: impl FnOnce(&mut Value), corrupt: bool) {
    edit_bundle(path, |manifest, members| {
        change(manifest);
        if let Some(unknown) = manifest["calculations"][0]["artifacts"].get("future") {
            let path = unknown["path"].as_str().unwrap();
            if !members.iter().any(|(name, _)| name == path) {
                let original = manifest["calculations"][0]["artifacts"]["ao_eri"]["path"]
                    .as_str()
                    .unwrap();
                let payload = members
                    .iter()
                    .find(|(name, _)| name == original)
                    .unwrap()
                    .1
                    .clone();
                members.push((path.to_owned(), payload));
            }
        }
        if corrupt {
            let payload = manifest["calculations"][0]["artifacts"]["ao_eri"]["path"]
                .as_str()
                .unwrap();
            let (_, bytes) = members
                .iter_mut()
                .find(|(name, _)| name == payload)
                .unwrap();
            bytes[0] ^= 1;
        }
    });
}

#[test]
fn strict_verification_reports_skips_and_prioritizes_failures() {
    let directory = fixture();
    let root = directory.path();
    calculate(root, &["--artifact", "verify.rustiq"]);
    let original = fs::read(root.join("verify.rustiq")).unwrap();
    for unsupported_representation in [false, true] {
        fs::write(root.join("verify.rustiq"), &original).unwrap();
        rewrite_verification_bundle(
            &root.join("verify.rustiq"),
            |manifest| {
                let artifacts = manifest["calculations"][0]["artifacts"]
                    .as_object_mut()
                    .unwrap();
                if unsupported_representation {
                    artifacts.get_mut("ao_eri").unwrap()["representation"] = json!("future-v2");
                } else {
                    let mut unknown = artifacts["ao_eri"].clone();
                    unknown["representation"] = json!("future-v2");
                    let path = unknown["path"].as_str().unwrap();
                    unknown["path"] =
                        json!(format!("{}/future.bin", path.rsplit_once('/').unwrap().0));
                    artifacts.insert("future".into(), unknown);
                }
            },
            false,
        );
        for strict in [false, true] {
            let mut args = vec!["artifact", "verify", "verify.rustiq"];
            if strict {
                args.push("--strict");
            }
            let output = command(root, &args);
            assert_eq!(
                output.status.code(),
                Some(if strict { 5 } else { 0 }),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_verification(
                &output,
                if strict { 5 } else { 0 },
                "Verification incomplete",
                [1 + usize::from(!unsupported_representation), 0, 1],
            );
            let stderr = String::from_utf8_lossy(&output.stderr);
            for text in [
                "rustiq::artifact::verification_skipped",
                "calculation 0",
                "future-v2",
                "payload was not checked",
            ] {
                assert!(
                    stderr
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .contains(text),
                    "missing {text}: {stderr}"
                );
            }
            assert!(stderr.contains(if unsupported_representation {
                "unsupported representation"
            } else {
                "unknown artifact"
            }));
            assert!(!stderr.contains("Error:"), "{stderr}");
            if strict {
                assert!(stderr.contains("Warning:"), "{stderr}");
                assert!(stderr.contains("rustiq::artifact::verification_incomplete"));
            }
        }
        if !unsupported_representation {
            rewrite_verification_bundle(&root.join("verify.rustiq"), |_| {}, true);
            let output = command(root, &["artifact", "verify", "verify.rustiq", "--strict"]);
            assert_eq!(
                output.status.code(),
                Some(1),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("Failed:       1"));
        }
    }
    fs::write(root.join("verify.rustiq"), &original).unwrap();
    assert!(
        command(root, &["artifact", "verify", "verify.rustiq", "--strict"])
            .status
            .success()
    );
}

fn assert_verification(output: &Output, code: i32, status: &str, counts: [usize; 3]) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(code), "{stdout}\n{stderr}");
    assert!(stdout.starts_with(status), "{stdout}");
    assert!(stdout.contains("  Calculations: 1\n"), "{stdout}");
    for (label, count) in ["Verified:    ", "Failed:      ", "Skipped:     "]
        .into_iter()
        .zip(counts)
    {
        assert!(stdout.contains(&format!("  {label} {count}\n")), "{stdout}");
    }
    assert!(!stdout.contains("rustiq::"));
    assert!(!stderr.contains("Calculations:"));
    assert!(!stdout.contains('\u{1b}') && !stderr.contains('\u{1b}'));
}

// Rewrite ZIP members while preserving a valid container and CRCs. Tests can then
// distinguish payload integrity errors from decoding/validation errors.
fn edit_bundle(path: &Path, edit: impl FnOnce(&mut Value, &mut Vec<(String, Vec<u8>)>)) {
    use std::io::{Read, Write};
    let mut archive = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    let mut members = Vec::new();
    for index in 0..archive.len() {
        let mut member = archive.by_index(index).unwrap();
        let mut bytes = Vec::new();
        member.read_to_end(&mut bytes).unwrap();
        members.push((member.name().unwrap().into_owned(), bytes));
    }
    drop(archive);
    let index = members
        .iter()
        .position(|(name, _)| name == "manifest.json")
        .unwrap();
    let mut manifest = serde_json::from_slice(&members[index].1).unwrap();
    edit(&mut manifest, &mut members);
    members[index].1 = serde_json::to_vec(&manifest).unwrap();
    let mut writer = zip::ZipWriter::new(fs::File::create(path).unwrap());
    for (name, bytes) in members {
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&bytes).unwrap();
    }
    writer.finish().unwrap();
}

fn hf_bundle(root: &Path, unrestricted: bool) {
    use rustiq_core::{
        config::{HfConfig, HfMethod},
        persistence::{ConvergedHf, ConvergedHfArtifact},
        prelude::*,
    };
    let geometry = Geometry::from_source("h2.xyz", "2\nH2\nH 0 0 0\nH 1.4 0 0\n").unwrap();
    let basis = BasisFile::from_reader(&include_bytes!("data/sto-3g.json")[..]).unwrap();
    let mut hf = HfConfig::default();
    hf.method = if unrestricted {
        HfMethod::Uhf
    } else {
        HfMethod::Rhf
    }
    .into();
    let prepared = CalculationBuilder::new(&geometry, &basis)
        .with_hf(hf)
        .prepare()
        .unwrap();
    let solution = prepared.run_hf().unwrap().into_converged().unwrap();
    let mut data = RustiQData::from_calculation(&prepared).unwrap();
    data.set::<ConvergedHfArtifact>(&ConvergedHf::from_solution(&solution).unwrap())
        .unwrap();
    data.write_with_eri(root.join("hf.rustiq"), solution.ao_eri())
        .unwrap();
}

#[test]
fn verification_reads_rhf_uhf_and_skips_unknown_npy_artifact() {
    for unrestricted in [false, true] {
        let directory = fixture();
        let root = directory.path();
        hf_bundle(root, unrestricted);
        for legacy in [false, true] {
            if legacy {
                edit_bundle(&root.join("hf.rustiq"), |manifest, _| {
                    manifest["calculations"][0]["artifacts"]["converged_hf"]["representation"] =
                        json!("rustiq-converged-hf-npy-v1");
                });
            }
            for strict in [false, true] {
                let mut args = vec!["artifact", "verify", "hf.rustiq"];
                if strict {
                    args.push("--strict");
                }
                let output = command(root, &args);
                if legacy {
                    assert_verification(
                        &output,
                        if strict { 5 } else { 0 },
                        "Verification incomplete",
                        [1, 0, 1],
                    );
                } else {
                    assert_verification(&output, 0, "Verification successful", [2, 0, 0]);
                }
                if !legacy {
                    assert_eq!(output.stderr, b"");
                }
            }
        }
    }
}

#[test]
fn verification_collects_independent_failures_and_retains_integrity_paths() {
    let directory = fixture();
    let root = directory.path();
    hf_bundle(root, false);
    edit_bundle(&root.join("hf.rustiq"), |manifest, members| {
        for name in ["ao_eri", "converged_hf"] {
            let path = manifest["calculations"][0]["artifacts"][name]["path"]
                .as_str()
                .unwrap();
            members.iter_mut().find(|(name, _)| name == path).unwrap().1[0] ^= 1;
        }
    });
    for strict in [false, true] {
        let mut args = vec!["artifact", "verify", "hf.rustiq"];
        if strict {
            args.push("--strict");
        }
        let output = command(root, &args);
        assert_verification(&output, 1, "Verification failed", [0, 2, 0]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Error:"), "{stderr}");
        assert!(stderr.contains("rustiq::artifact::verification_batch_failed"));
        assert_eq!(
            stderr
                .matches("rustiq::artifact::verification_failed")
                .count(),
            2,
            "{stderr}"
        );
        for text in [
            "calculation 0",
            "artifact ao_eri",
            "artifact converged_hf",
            "integrity check failed",
            "ao-eri.npy",
            "converged-hf.json",
        ] {
            assert!(
                stderr
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .contains(text),
                "missing {text}: {stderr}"
            );
        }
    }
}

#[test]
fn verification_reports_a_size_mismatch_and_checks_the_remaining_artifact() {
    let directory = fixture();
    let root = directory.path();
    hf_bundle(root, false);
    let path = root.join("hf.rustiq");
    edit_bundle(&path, |manifest, _| {
        let size = &mut manifest["calculations"][0]["artifacts"]["ao_eri"]["size"];
        *size = json!(size.as_u64().unwrap() + 1);
    });
    assert!(RustiQBundle::open(&path).is_err());

    let text = command(
        root,
        &["artifact", "verify", "hf.rustiq", "--format", "text"],
    );
    assert_verification(&text, 1, "Verification failed", [1, 1, 0]);

    let output = command(
        root,
        &[
            "artifact",
            "verify",
            "hf.rustiq",
            "--strict",
            "--format",
            "json",
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "failed");
    assert_eq!(report["artifacts"][0]["status"], "invalid");
    assert_eq!(
        report["artifacts"][0]["code"],
        "rustiq::artifact::verification_failed"
    );
    assert_eq!(report["artifacts"][1]["status"], "valid");
    assert!(report["artifacts"][0]["errors"][0]
        .as_str()
        .unwrap()
        .contains("integrity check failed"));
}

#[test]
fn verification_decoding_failure_is_not_a_skip() {
    let directory = fixture();
    let root = directory.path();
    hf_bundle(root, false);
    edit_bundle(&root.join("hf.rustiq"), |manifest, members| {
        let artifact = &mut manifest["calculations"][0]["artifacts"]["converged_hf"];
        let path = artifact["path"].as_str().unwrap();
        let bytes = &mut members.iter_mut().find(|(name, _)| name == path).unwrap().1;
        *bytes = b"invalid JSON".to_vec();
        artifact["size"] = json!(bytes.len());
        artifact["digest"] = serde_json::to_value(rustiq_core::persistence::sha256(bytes)).unwrap();
    });
    let output = command(root, &["artifact", "verify", "hf.rustiq"]);
    assert_verification(&output, 1, "Verification failed", [1, 1, 0]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid artifact metadata"), "{stderr}");
    assert!(!stderr.contains("integrity check failed"), "{stderr}");
}

#[test]
fn verification_empty_and_fatal_archives() {
    let directory = fixture();
    let root = directory.path();
    calculate(root, &["--artifact", "source.rustiq"]);
    let source = RustiQData::open(root.join("source.rustiq")).unwrap();
    RustiQData::from_calculation(&source.prepare_calculation().unwrap())
        .unwrap()
        .write(root.join("empty.rustiq"))
        .unwrap();
    for strict in [false, true] {
        let mut args = vec!["artifact", "verify", "empty.rustiq"];
        if strict {
            args.push("--strict");
        }
        let output = command(root, &args);
        assert_verification(&output, 0, "Verification successful", [0, 0, 0]);
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("no numerical payload was verified")
        );
        assert_eq!(output.stderr, b"");
    }
    fs::write(root.join("invalid.rustiq"), b"invalid ZIP").unwrap();
    let output = command(root, &["artifact", "verify", "invalid.rustiq"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, b"");
    assert!(String::from_utf8_lossy(&output.stderr).contains("Error:"));
}

#[test]
fn verification_skips_legacy_inline_hf_representation() {
    let directory = fixture();
    let root = directory.path();
    hf_bundle(root, false);
    edit_bundle(&root.join("hf.rustiq"), |manifest, _| {
        manifest["calculations"][0]["artifacts"]["converged_hf"]["representation"] =
            json!("rustiq-converged-hf-v1");
    });
    let output = command(root, &["artifact", "verify", "hf.rustiq"]);
    assert_verification(&output, 0, "Verification incomplete", [1, 0, 1]);
}

#[test]
fn verification_mixes_valid_skipped_and_failed_artifacts() {
    let directory = fixture();
    let root = directory.path();
    hf_bundle(root, false);
    rewrite_verification_bundle(
        &root.join("hf.rustiq"),
        |manifest| {
            let artifacts = manifest["calculations"][0]["artifacts"]
                .as_object_mut()
                .unwrap();
            let mut unknown = artifacts["ao_eri"].clone();
            let path = unknown["path"].as_str().unwrap();
            unknown["path"] = json!(format!("{}/future.bin", path.rsplit_once('/').unwrap().0));
            unknown["representation"] = json!("future-v2");
            artifacts.insert("future".into(), unknown);
        },
        true,
    );
    for strict in [false, true] {
        let mut args = vec!["artifact", "verify", "hf.rustiq"];
        if strict {
            args.push("--strict");
        }
        let output = command(root, &args);
        assert_verification(&output, 1, "Verification failed", [1, 1, 1]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("rustiq::artifact::verification_skipped"));
        assert!(stderr.contains("rustiq::artifact::verification_failed"));
        assert!(!stderr.contains("rustiq::artifact::verification_incomplete"));
    }
}

#[test]
fn verification_respects_explicit_color_modes() {
    let directory = fixture();
    let root = directory.path();
    calculate(root, &["--artifact", "verify.rustiq"]);
    rewrite_verification_bundle(
        &root.join("verify.rustiq"),
        |manifest| {
            manifest["calculations"][0]["artifacts"]["ao_eri"]["representation"] =
                json!("future-v2");
        },
        false,
    );
    for color in ["never", "always"] {
        let output = Command::new(env!("CARGO_BIN_EXE_rustiq"))
            .current_dir(root)
            .env("NO_COLOR", "1")
            .env("RUSTIQ_COLOR", "never")
            .env("TERM", "xterm-256color")
            .args(["--color", color, "artifact", "verify", "verify.rustiq"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(stderr.contains('\u{1b}'), color == "always", "{stderr}");
        assert!(String::from_utf8_lossy(&output.stdout).starts_with("Verification incomplete"));
    }
}

#[test]
fn verification_json_lists_valid_invalid_unknown_and_unsupported_artifacts() {
    let directory = fixture();
    let root = directory.path();
    hf_bundle(root, false);
    rewrite_verification_bundle(
        &root.join("hf.rustiq"),
        |manifest| {
            let artifacts = manifest["calculations"][0]["artifacts"]
                .as_object_mut()
                .unwrap();
            let mut unknown = artifacts["ao_eri"].clone();
            let path = unknown["path"].as_str().unwrap();
            unknown["path"] = json!(format!("{}/future.bin", path.rsplit_once('/').unwrap().0));
            unknown["representation"] = json!("future-v2");
            artifacts.insert("future".into(), unknown);
            artifacts.get_mut("converged_hf").unwrap()["representation"] = json!("future-v2");
        },
        false,
    );

    let original = fs::read(root.join("hf.rustiq")).unwrap();
    let schema: Value = serde_json::from_str(include_str!(
        "../schemas/artifact-verification-v1.schema.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    for (corrupt, status, code, ao_status) in [
        (false, "incomplete", 5, "valid"),
        (true, "failed", 1, "invalid"),
    ] {
        fs::write(root.join("hf.rustiq"), &original).unwrap();
        if corrupt {
            rewrite_verification_bundle(&root.join("hf.rustiq"), |_| {}, true);
        }
        let output = command(
            root,
            &[
                "artifact",
                "verify",
                "hf.rustiq",
                "--strict",
                "--format",
                "json",
            ],
        );
        assert_eq!(
            output.status.code(),
            Some(code),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        validator.validate(&report).unwrap();
        assert_eq!(report["status"], status);
        assert_eq!(report["calculation_count"], 1);
        assert_eq!(report["verified"], usize::from(!corrupt));
        assert_eq!(report["failed"], usize::from(corrupt));
        assert_eq!(report["skipped"], 2);
        let entries = report["artifacts"].as_array().unwrap();
        assert_eq!(entries.len(), 3);
        for (entry, (name, status)) in entries.iter().zip([
            ("ao_eri", ao_status),
            ("converged_hf", "unsupported_representation"),
            ("future", "unknown_artifact"),
        ]) {
            assert_eq!(entry["calculation_index"], 0);
            assert_eq!(entry["name"], name);
            assert_eq!(entry["status"], status);
        }
        if corrupt {
            assert!(entries[0]["errors"][0]
                .as_str()
                .unwrap()
                .contains("integrity check failed"));
        }
    }
}

fn post_scf(root: &Path, bundle: &str, extra: &[&str]) -> Output {
    let mut args = vec![
        "run",
        bundle,
        "--post-scf",
        "postscf.toml",
        "--format",
        "json",
    ];
    args.extend_from_slice(extra);
    command(root, &args)
}

fn post_scf_report(output: &Output) -> Value {
    let decoded = serde_json::from_slice(&output.stdout);
    assert!(
        decoded.is_ok(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = decoded.unwrap();
    let schema: Value =
        serde_json::from_str(include_str!("../schemas/post-scf-output-v1.schema.json")).unwrap();
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(&report)
        .unwrap();
    report
}

fn post_scf_fixture() -> (TempDir, Vec<Value>) {
    let dir = fixture();
    let root = dir.path();
    let mut entries = Vec::new();
    let mut fresh = Vec::new();
    for (index, settings) in [
        "[method.mp2]",
        "[method.hf]\nmax_iterations = 1",
        "[method.hf]\nmethod = 'Uhf'\n[method.mp2]",
        "",
    ]
    .into_iter()
    .enumerate()
    {
        fs::write(
            root.join("run.toml"),
            format!("[basis]\nname = 'sto-3g'\n{settings}\n"),
        )
        .unwrap();
        let name = format!("entry-{index}.rustiq");
        let output = command(
            root,
            &["run", "run.toml", "--artifact", &name, "--format", "json"],
        );
        assert_eq!(output.status.code(), Some(if index == 1 { 3 } else { 0 }));
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        fresh.push(result["calculation"]["mp2"].clone());
        entries.push(RustiQData::open(root.join(name)).unwrap());
    }
    let path = root.join("campaign.rustiq");
    let mut campaign = RustiQBundle::new(entries).unwrap();
    campaign
        .add_source("campaign.txt", b"campaign source")
        .unwrap();
    campaign.write(&path).unwrap();
    edit_bundle(&path, |manifest, _| {
        manifest["calculations"][3]["artifacts"]
            .as_object_mut()
            .unwrap()
            .remove("ao_eri");
    });
    fs::remove_file(root.join("molecule.xyz")).unwrap();
    fs::remove_file(root.join("run.toml")).unwrap();
    fs::remove_dir_all(root.join("data")).unwrap();
    fs::write(
        root.join("postscf.toml"),
        include_str!("../samples/h2/sto-3g/postscf.toml"),
    )
    .unwrap();

    (dir, fresh)
}

#[test]
fn post_scf_campaign_keeps_indices_and_continues_past_missing_references() {
    let (dir, fresh) = post_scf_fixture();
    let root = dir.path();
    let out = post_scf(root, "campaign.rustiq", &["--pretty"]);
    let value = post_scf_report(&out);
    assert!(out.status.success());
    assert_eq!(
        value["summary"],
        json!({"succeeded":2,"skipped":2,"failed":0})
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(stderr.matches("rustiq::post_scf::skipped").count(), 2);
    assert!(stderr.contains("Calculation 1, method mp2: no persisted converged HF"));
    assert!(stderr.contains("Calculation 3, method mp2: required artifact is missing: ao_eri"));
    for (index, calculation) in value["calculations"].as_array().unwrap().iter().enumerate() {
        assert_eq!(calculation["source_index"], index);
        let method = &calculation["methods"][0];
        assert_eq!(method["method"], "mp2");
        if index % 2 == 0 {
            assert_eq!(method["status"], "succeeded");
            for field in ["correlation_energy", "electronic_energy", "total_energy"] {
                approx::assert_abs_diff_eq!(
                    method["result"][field].as_f64().unwrap(),
                    fresh[index][field].as_f64().unwrap(),
                    epsilon = 1e-12,
                );
            }
        } else {
            assert_eq!(method["status"], "skipped");
            assert_eq!(method["diagnostic"]["code"], "rustiq::post_scf::skipped");
        }
    }
}

#[test]
fn post_scf_exports_selected_reference_and_preserves_source() {
    let (dir, _) = post_scf_fixture();
    let root = dir.path();
    let path = root.join("campaign.rustiq");
    let bytes = fs::read(&path).unwrap();
    let out = post_scf(
        root,
        "campaign.rustiq",
        &[
            "--calculation",
            "2",
            "--method",
            "mp2",
            "--method",
            "mp2",
            "--artifact",
            "selected.rustiq",
        ],
    );
    let value = post_scf_report(&out);
    assert!(out.status.success());
    assert_eq!(out.stderr, b"");
    assert_eq!(value["summary"]["succeeded"], 1);
    assert_eq!(value["calculations"][0]["source_index"], 2);
    assert_eq!(
        value["calculations"][0]["methods"][0]["result"]["reference"],
        "UHF"
    );

    let selected = RustiQBundle::open(root.join("selected.rustiq")).unwrap();
    assert_eq!(selected.calculations().len(), 1);
    let copied_source = selected
        .sources()
        .find(|source| source.original_name() == "campaign.txt")
        .unwrap();
    assert_eq!(copied_source.bytes(), b"campaign source");
    assert_eq!(
        selected.calculations()[0].resolved_hf_method(),
        RustiQBundle::open(root.join("entry-2.rustiq"))
            .unwrap()
            .calculations()[0]
            .resolved_hf_method()
    );

    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn post_scf_force_replaces_other_destinations_but_never_the_source() {
    let (directory, _) = post_scf_fixture();
    let root = directory.path();
    let source = root.join("campaign.rustiq");
    let original_source = fs::read(&source).unwrap();
    let destination = root.join("existing.rustiq");
    fs::write(&destination, b"previous output").unwrap();

    let rejected = post_scf(root, "campaign.rustiq", &["--artifact", "existing.rustiq"]);
    assert!(!rejected.status.success());
    assert_eq!(fs::read(&destination).unwrap(), b"previous output");

    let forced = post_scf(
        root,
        "campaign.rustiq",
        &["--artifact", "existing.rustiq", "--force"],
    );
    assert!(
        forced.status.success(),
        "{}",
        String::from_utf8_lossy(&forced.stderr)
    );
    assert!(RustiQBundle::open(&destination).is_ok());

    for forbidden in ["campaign.rustiq", "linked-source.rustiq"] {
        if forbidden == "linked-source.rustiq" {
            fs::hard_link(&source, root.join(forbidden)).unwrap();
        }
        let rejected = post_scf(
            root,
            "campaign.rustiq",
            &["--artifact", forbidden, "--force"],
        );
        assert!(!rejected.status.success());
        assert!(String::from_utf8_lossy(&rejected.stderr).contains("must not replace"));
    }
    assert_eq!(fs::read(&source).unwrap(), original_source);
}

#[cfg(unix)]
#[test]
fn post_scf_force_preserves_symbolic_output_alias() {
    let (directory, _) = post_scf_fixture();
    let root = directory.path();
    fs::write(root.join("target.rustiq"), b"previous output").unwrap();
    std::os::unix::fs::symlink("target.rustiq", root.join("alias.rustiq")).unwrap();

    let rejected = post_scf(root, "campaign.rustiq", &["--artifact", "alias.rustiq"]);
    assert!(!rejected.status.success());
    assert_eq!(
        fs::read(root.join("target.rustiq")).unwrap(),
        b"previous output"
    );

    let forced = post_scf(
        root,
        "campaign.rustiq",
        &["--artifact", "alias.rustiq", "--force"],
    );
    assert!(
        forced.status.success(),
        "{}",
        String::from_utf8_lossy(&forced.stderr)
    );
    assert!(fs::symlink_metadata(root.join("alias.rustiq"))
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(RustiQBundle::open(root.join("target.rustiq")).is_ok());
}

#[test]
fn post_scf_text_reports_mp2_without_running_scf() {
    let (dir, _) = post_scf_fixture();
    let root = dir.path();
    let out = command(
        root,
        &[
            "run",
            "campaign.rustiq",
            "--post-scf",
            "postscf.toml",
            "--calculation",
            "0",
        ],
    );
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("MP2 correlation energy:"));
    assert!(text.contains("1 succeeded, 0 skipped, 0 failed"));
    assert!(!text.contains("SCF iterations"));
}

#[test]
fn post_scf_rejects_selections_without_a_successful_calculation() {
    let (dir, _) = post_scf_fixture();
    let root = dir.path();
    let out = post_scf(root, "campaign.rustiq", &["--calculation", "1"]);
    let value = post_scf_report(&out);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        value["summary"],
        json!({"succeeded":0,"skipped":1,"failed":0})
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    let warning = stderr.find("rustiq::post_scf::skipped").unwrap();
    assert!(
        warning
            < stderr
                .find("rustiq::post_scf::no_successful_calculations")
                .unwrap()
    );

    let out = post_scf(root, "campaign.rustiq", &["--calculation", "4"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("zero-based index from 0 to 3"));
}

#[test]
fn post_scf_pretty_json_is_machine_readable_with_forced_color() {
    let dir = fixture();
    let root = dir.path();
    hf_bundle(root, false);
    fs::write(root.join("postscf.toml"), "[method.mp2]").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rustiq"))
        .current_dir(root)
        .args([
            "--color",
            "always",
            "run",
            "hf.rustiq",
            "--post-scf",
            "postscf.toml",
            "--format",
            "json",
            "--pretty",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stdout.starts_with(b"{\n  "));
    assert_eq!(post_scf_report(&out)["summary"]["succeeded"], 1);
}

#[test]
fn post_scf_publishes_successes_with_unrelated_corrupt_payload() {
    let (dir, _) = post_scf_fixture();
    let root = dir.path();
    let path = root.join("campaign.rustiq");
    edit_bundle(&path, |manifest, members| {
        let artifacts = manifest["calculations"][0]["artifacts"]
            .as_object_mut()
            .unwrap();
        let mut unrelated = artifacts["ao_eri"].clone();
        unrelated["path"] = json!("calculations/calculation-0/arrays/unrelated.npy");
        unrelated["attributes"] = json!({});
        unrelated["representation"] = json!("future-v1");
        let size = unrelated["size"].as_u64().unwrap();
        members.push((
            "calculations/calculation-0/arrays/unrelated.npy".into(),
            vec![0; usize::try_from(size).unwrap()],
        ));
        artifacts.insert("unrelated".into(), unrelated);
    });
    let source = RustiQBundle::open_for_restore(&path).unwrap();
    let mut copy = RustiQBundle::new(vec![source.calculations()[0].clone()]).unwrap();
    assert!(copy.write(root.join("unfiltered.rustiq")).is_err());
    assert!(!root.join("unfiltered.rustiq").exists());
    let original = fs::read(&path).unwrap();
    let output = post_scf(root, "campaign.rustiq", &["--artifact", "export.rustiq"]);
    let report = post_scf_report(&output);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let saved = RustiQBundle::open(root.join("export.rustiq")).unwrap();
    assert_eq!(saved.calculations().len(), 2);
    for (entry, source_index) in saved.calculations().iter().zip([0, 2]) {
        let result = &report["calculations"][source_index];
        assert_eq!(result["source_index"], source_index);
        assert_eq!(result["methods"][0]["status"], "succeeded");
        assert!(entry.get::<ConvergedHfArtifact>().unwrap().is_some());
        assert!(entry.get::<AoEriArtifact>().unwrap().is_some());
        assert_eq!(entry.artifact_representations().count(), 2);
        assert_eq!(
            entry.resolved_hf_method(),
            RustiQData::open(root.join(format!("entry-{source_index}.rustiq")))
                .unwrap()
                .resolved_hf_method()
        );
    }
    assert_eq!(fs::read(path).unwrap(), original);
}

#[test]
fn post_scf_reports_corrupt_payloads_as_failures_and_preserves_successful_pairs() {
    let (dir, _) = post_scf_fixture();
    let root = dir.path();
    let path = root.join("campaign.rustiq");
    for (index, corrupt_size) in [(0, false), (2, true)] {
        edit_bundle(&path, |manifest, members| {
            let payload = manifest["calculations"][index]["artifacts"]["ao_eri"]["path"]
                .as_str()
                .unwrap();
            let (_, data) = members
                .iter_mut()
                .find(|(name, _)| name == payload)
                .unwrap();
            if corrupt_size {
                data.pop().unwrap();
            } else {
                data[0] ^= 1;
            }
        });
        let artifact = format!("export-{index}.rustiq");
        let out = post_scf(root, "campaign.rustiq", &["--artifact", &artifact]);
        if index == 0 {
            let saved = RustiQBundle::open(root.join(&artifact)).unwrap();
            assert_eq!(saved.calculations().len(), 1);
        } else {
            assert!(!root.join(artifact).exists());
        }
        assert_eq!(out.status.code(), Some(if index == 0 { 4 } else { 1 }));
        let value = post_scf_report(&out);
        assert_eq!(value["summary"]["failed"], if index == 0 { 1 } else { 2 });
        assert_eq!(
            value["calculations"][index]["methods"][0]["status"],
            "failed"
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains(&format!("Calculation {index}, method mp2")));
        assert!(stderr.contains("integrity check failed"));
    }
}

#[test]
fn post_scf_distinguishes_execution_errors_from_global_configuration_errors() {
    let dir = fixture();
    let root = dir.path();
    hf_bundle(root, false);
    for (config, message) in [
        ("[method.mp2]\nfrozen_orbitals = 1", "frozen orbitals"),
        (
            "[method.mp2]\nmemory_limit = '1 B'",
            "MP2 workspace requires",
        ),
    ] {
        fs::write(root.join("postscf.toml"), config).unwrap();
        let out = post_scf(root, "hf.rustiq", &[]);
        let value = post_scf_report(&out);
        assert_eq!(out.status.code(), Some(1));
        assert_eq!(
            value["summary"],
            json!({"succeeded":0,"skipped":0,"failed":1})
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains("rustiq::post_scf::no_successful_calculations"));
        assert!(stderr.contains(message), "{stderr}");
        assert!(!stderr.contains("rustiq::post_scf::skipped"));
    }
    for (config, args, message) in [
        ("", &[][..], "No Post-HF methods are configured"),
        ("", &["--method", "mp2"][..], "is not configured"),
        ("[method.mp2]", &["--method", "ccsd"][..], "is unavailable"),
        ("[method.ccsd]", &[][..], "ccsd"),
        ("[method.hf]", &[][..], "hf"),
        ("[molecule]", &[][..], "molecule"),
        (
            "[method.mp2]\nmemory_limit = 'bad'",
            &[][..],
            "memory limit",
        ),
        (
            "[method.mp2]\nfrozen_orbitals = -1",
            &[][..],
            "frozen_orbitals",
        ),
    ] {
        fs::write(root.join("postscf.toml"), config).unwrap();
        let out = post_scf(root, "hf.rustiq", args);
        assert!(!out.status.success());
        assert_eq!(out.stdout, b"");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains(message), "{stderr}");
        assert!(!stderr.contains("rustiq::post_scf::skipped"));
    }
}

#[test]
fn post_scf_artifact_requested_matches_direct_nickel_study() {
    let dir = fixture();
    let root = dir.path();
    calculate(root, &["--artifact", "hf.rustiq"]);
    for (index, (toml, nickel)) in [
        ("", "{}"),
        (
            "memory_limit = '128 MiB'\nfrozen_orbitals = 0",
            "{memory_limit = \"128 MiB\", frozen_orbitals = 0}",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        fs::write(root.join("postscf.toml"), format!("[method.mp2]\n{toml}\n")).unwrap();
        fs::write(
            root.join("study.ncl"),
            format!("[{{basis.name = \"sto-3g\", method.mp2 = {nickel}}}]"),
        )
        .unwrap();
        let direct = format!("direct-{index}.rustiq");
        let resumed = format!("resumed-{index}.rustiq");
        success(
            root,
            &[
                "run",
                "study.ncl",
                "--artifact",
                &direct,
                "--format",
                "json",
            ],
        );
        let out = post_scf(
            root,
            "hf.rustiq",
            &["--calculation", "0", "--artifact", &resumed],
        );
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let inspect = |path| success(root, &["artifact", "inspect", path, "--format", "json"]);
        assert_eq!(
            inspect(&direct)["calculations"][0]["requested"],
            inspect(&resumed)["calculations"][0]["requested"]
        );
        let verify = command(root, &["artifact", "verify", &resumed, "--strict"]);
        assert!(
            verify.status.success(),
            "{}",
            String::from_utf8_lossy(&verify.stderr)
        );
        let existing = fs::read(root.join(&resumed)).unwrap();
        assert!(!post_scf(root, "hf.rustiq", &["--artifact", &resumed])
            .status
            .success());
        assert_eq!(fs::read(root.join(&resumed)).unwrap(), existing);
    }
}

#[test]
fn post_scf_split_run_matches_full_mp2_calculation() {
    for (method, reference) in [("Rhf", "RHF"), ("Uhf", "UHF")] {
        assert_post_scf_split_run_matches_full_mp2_calculation(method, reference);
    }
}

fn assert_post_scf_split_run_matches_full_mp2_calculation(method: &str, reference: &str) {
    let dir = fixture();
    let root = dir.path();
    let runfile = root.join("run.toml");
    let mut base_config = fs::read_to_string(&runfile).unwrap();
    if reference == "UHF" {
        fs::write(
            root.join("molecule.xyz"),
            include_str!("../samples/oh/oh.xyz"),
        )
        .unwrap();
        base_config = base_config.replace("[molecule]\n", "[molecule]\nmultiplicity = 2\n");
    } else {
        fs::write(
            root.join("molecule.xyz"),
            include_str!("../samples/h2o/h2o.xyz"),
        )
        .unwrap();
    }
    let hf_config = format!("{base_config}\n[method.hf]\nmethod = '{method}'\n");
    let mp2_config = include_str!("../samples/h2/sto-3g/postscf.toml");

    fs::write(&runfile, format!("{hf_config}\n{mp2_config}")).unwrap();
    let full = calculate(root, &["--artifact", "full.rustiq"]);

    fs::write(&runfile, hf_config).unwrap();
    fs::write(root.join("postscf.toml"), mp2_config).unwrap();
    let hf = calculate(root, &["--artifact", "hf.rustiq"]);
    assert!(hf["calculation"].get("mp2").is_none());
    assert_eq!(full["calculation"]["hf"], hf["calculation"]["hf"]);

    fs::remove_file(runfile).unwrap();
    fs::remove_file(root.join("molecule.xyz")).unwrap();
    fs::remove_dir_all(root.join("data")).unwrap();
    let source = fs::read(root.join("hf.rustiq")).unwrap();
    let output = post_scf(root, "hf.rustiq", &["--artifact", "resumed.rustiq"]);
    assert_eq!(fs::read(root.join("hf.rustiq")).unwrap(), source);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = post_scf_report(&output);
    assert_eq!(
        report["summary"],
        json!({"succeeded": 1, "skipped": 0, "failed": 0})
    );
    let method = &report["calculations"][0]["methods"][0];
    assert_eq!(method["method"], "mp2");
    assert_eq!(method["status"], "succeeded");
    let resumed = &method["result"];
    assert_eq!(full["calculation"]["hf"]["method"], reference);
    assert_eq!(resumed["reference"], reference);
    for field in ["correlation_energy", "electronic_energy", "total_energy"] {
        approx::assert_abs_diff_eq!(
            resumed[field].as_f64().unwrap(),
            full["calculation"]["mp2"][field].as_f64().unwrap(),
            epsilon = 1e-12,
        );
    }
    assert_eq!(
        resumed["nuclear_repulsion_energy"],
        full["calculation"]["hf"]["nuclear_repulsion_energy"]
    );

    let inspect = |path| success(root, &["artifact", "inspect", path, "--format", "json"]);
    assert_eq!(
        inspect("full.rustiq")["calculations"][0]["requested"],
        inspect("resumed.rustiq")["calculations"][0]["requested"]
    );

    let archived_hf = |path| {
        RustiQData::open(root.join(path))
            .unwrap()
            .get::<ConvergedHfArtifact>()
            .unwrap()
            .unwrap()
    };
    let full_hf = archived_hf("full.rustiq");
    let resumed_hf = archived_hf("resumed.rustiq");
    assert_eq!(full_hf.identity(), resumed_hf.identity());
    assert_eq!(full_hf.energies(), resumed_hf.energies());
    assert_eq!(full_hf.iterations(), resumed_hf.iterations());
    assert_eq!(full_hf.convergence(), resumed_hf.convergence());
    assert_eq!(full_hf.spin(), resumed_hf.spin());
}

#[test]
fn molden_export_api_matches_cli_output() {
    let directory = fixture();
    let root = directory.path();
    hf_bundle(root, false);
    let data = RustiQData::open(root.join("hf.rustiq")).unwrap();
    let cli = command(
        root,
        &["artifact", "export", "hf.rustiq", "--format", "molden"],
    );
    assert!(
        cli.status.success(),
        "{}",
        String::from_utf8_lossy(&cli.stderr)
    );

    let mut output = Vec::new();
    data.export(ExportFormat::Molden)
        .unwrap()
        .write_to(&mut output)
        .unwrap();
    assert_eq!(output, cli.stdout);

    let prepared = data.prepare_calculation().unwrap();
    let missing = RustiQData::from_calculation(&prepared).unwrap();
    assert!(matches!(
        missing.export(ExportFormat::Molden),
        Err(ExportError::MissingOrbitals)
    ));
}

#[test]
fn molden_exports_one_selected_calculation_without_ao_eris() {
    let directory = fixture();
    let root = directory.path();
    hf_bundle(root, false);
    let hf = RustiQData::open(root.join("hf.rustiq")).unwrap();
    let orbitals = hf.get::<ConvergedHfArtifact>().unwrap().unwrap();
    let prepared = hf.prepare_calculation().unwrap();
    let mut first = RustiQData::from_calculation(&prepared).unwrap();
    first.set::<ConvergedHfArtifact>(&orbitals).unwrap();
    first.write(root.join("orbitals.rustiq")).unwrap();

    let export = |bundle: &str, options: &[&str]| {
        let mut args = vec!["artifact", "export", bundle, "--format", "molden"];
        args.extend_from_slice(options);
        command(root, &args)
    };
    let rhf = export("orbitals.rustiq", &[]);
    assert!(
        rhf.status.success(),
        "{}",
        String::from_utf8_lossy(&rhf.stderr)
    );
    let text = String::from_utf8_lossy(&rhf.stdout);
    assert!(text.starts_with("[Molden Format]\n[Atoms] AU\n"));
    assert!(text.contains(" Spin= Alpha\n Occup= 2\n"));
    assert!(!text.contains("Spin= Beta"));

    fs::remove_file(root.join("hf.rustiq")).unwrap();
    hf_bundle(root, true);
    let second = RustiQData::open(root.join("hf.rustiq")).unwrap();
    let missing = RustiQData::from_calculation(&prepared).unwrap();
    RustiQBundle::new(vec![first, second, missing])
        .unwrap()
        .write(root.join("multi.rustiq"))
        .unwrap();
    let original = fs::read(root.join("multi.rustiq")).unwrap();
    let selected = export("multi.rustiq", &["--calculation", "0"]);
    assert!(selected.status.success());
    assert_eq!(selected.stdout, rhf.stdout);
    let selected = export("multi.rustiq", &["--calculation", "1"]);
    assert!(selected.status.success());
    let text = String::from_utf8_lossy(&selected.stdout);
    assert!(text.contains(" Spin= Alpha\n Occup= 1\n"));
    assert!(text.contains(" Spin= Beta\n Occup= 1\n"));
    assert_eq!(text.matches("[MO]").count(), 1);

    assert_molden_file_publication(root, export, &rhf.stdout, &selected.stdout, &original);

    for (options, message) in [
        (&[][..], "Use --calculation"),
        (&["--calculation", "3"][..], "index 3 is out of range"),
        (
            &["--calculation", "2"][..],
            "no saved converged HF orbitals",
        ),
    ] {
        let failure = export("multi.rustiq", options);
        assert_eq!(failure.status.code(), Some(1));
        assert_eq!(failure.stdout, b"");
        assert!(String::from_utf8_lossy(&failure.stderr).contains(message));
    }

    edit_bundle(&root.join("multi.rustiq"), |manifest, _| {
        manifest["calculations"][1]["artifacts"]["converged_hf"]["size"] = json!(0);
    });
    let unaffected = export("multi.rustiq", &["--calculation", "0"]);
    assert!(unaffected.status.success());
    assert_eq!(unaffected.stdout, rhf.stdout);
    let corrupt = export("multi.rustiq", &["--calculation", "1"]);
    assert_eq!(corrupt.status.code(), Some(1));
    assert_eq!(corrupt.stdout, b"");
    let stderr = String::from_utf8_lossy(&corrupt.stderr);
    assert!(stderr.contains("converged HF artifact for calculation 1"));
    assert!(stderr.contains("integrity check failed"));
}

fn assert_molden_file_publication(
    root: &Path,
    export: impl Fn(&str, &[&str]) -> Output,
    rhf: &[u8],
    uhf: &[u8],
    original: &[u8],
) {
    let output = root.join("orbitals.molden");
    for (index, option, expected) in [("1", "-o", uhf), ("0", "--output", rhf)] {
        let written = export(
            "multi.rustiq",
            &["--calculation", index, option, "orbitals.molden", "--force"],
        );
        assert!(written.status.success());
        assert_eq!(written.stdout, b"");
        assert_eq!(&fs::read(&output).unwrap(), expected);
    }

    let failure = export(
        "multi.rustiq",
        &["--calculation", "1", "-o", "orbitals.molden"],
    );
    assert_eq!(failure.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&failure.stderr).contains("--force"));
    assert_eq!(fs::read(&output).unwrap(), rhf);

    fs::create_dir(root.join("directory.molden")).unwrap();
    let before = fs::read_dir(root).unwrap().count();
    let failure = export(
        "multi.rustiq",
        &["--calculation", "0", "-o", "directory.molden"],
    );
    assert_eq!(failure.status.code(), Some(1));
    assert!(root.join("directory.molden").is_dir());
    assert_eq!(fs::read_dir(root).unwrap().count(), before);

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("multi.rustiq", root.join("alias.rustiq")).unwrap();
        let failure = export(
            "multi.rustiq",
            &["--calculation", "0", "-o", "alias.rustiq"],
        );
        assert_eq!(failure.status.code(), Some(1));
        assert!(
            String::from_utf8_lossy(&failure.stderr).contains("must not replace the input bundle")
        );
    }

    fs::hard_link(root.join("multi.rustiq"), root.join("linked.rustiq")).unwrap();
    for path in ["multi.rustiq", "linked.rustiq"] {
        let failure = export("multi.rustiq", &["--calculation", "0", "-o", path]);
        assert_eq!(failure.status.code(), Some(1));
        assert_eq!(failure.stdout, b"");
        assert!(
            String::from_utf8_lossy(&failure.stderr).contains("must not replace the input bundle")
        );
    }

    for path in ["missing.molden", "orbitals.molden"] {
        let missing = export("multi.rustiq", &["--calculation", "2", "-o", path]);
        assert_eq!(missing.status.code(), Some(1));
        assert_eq!(missing.stdout, b"");
    }
    assert!(!root.join("missing.molden").exists());
    assert_eq!(fs::read(&output).unwrap(), rhf);
    assert_eq!(fs::read(root.join("multi.rustiq")).unwrap(), original);
}

#[test]
fn offline_missing_basis_explains_how_to_get_it() {
    let dir = fixture();
    fs::remove_file(dir.path().join("data/rustiq/basis_sets/sto-3g.json")).unwrap();
    let output = command(dir.path(), &["run", "run.toml", "--no-auto-download"]);
    assert!(!output.status.success());
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(
        diagnostic.contains("rustiq::basis::missing_offline"),
        "{diagnostic}"
    );
    let (_, help) = diagnostic.split_once("help:").expect("missing import help");
    let help = help.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        help.contains("rustiq basis import <file.json>"),
        "{diagnostic}"
    );
    if cfg!(feature = "online") {
        assert!(
            help.contains("rustiq basis download <basis-name>"),
            "{diagnostic}"
        );
        assert!(help.contains("`--auto-download`"), "{diagnostic}");
        assert!(help.contains("`--no-auto-download`"), "{diagnostic}");
    } else {
        assert!(
            help.contains("This build has no online download support"),
            "{diagnostic}"
        );
        assert!(!help.contains("basis download"), "{diagnostic}");
        assert!(!help.contains("--auto-download"), "{diagnostic}");
    }
}

#[test]
fn uncoded_io_error_uses_the_cli_fallback_code() {
    let directory = fixture();
    let output = command(directory.path(), &["run", "missing.toml"]);
    assert_eq!(output.status.code(), Some(1));
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(
        diagnostic.contains("rustiq::unexpected_error"),
        "{diagnostic}"
    );
}
