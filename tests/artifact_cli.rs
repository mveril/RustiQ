#![allow(
    clippy::unwrap_used,
    reason = "Integration tests fail immediately on unexpected fixture errors"
)]

use rustiq_core::persistence::{AoEriArtifact, RustiQBundle, RustiQData};
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
    for data in bundle.calculations() {
        assert!(data.get::<AoEriArtifact>().unwrap().is_some());
        assert_eq!(data.artifact_representations().count(), 1);
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
fn verification_counts_current_eri_artifact() {
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
    assert!(text.contains("Verified:     1"), "{text}");
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
                [usize::from(!unsupported_representation), 0, 1],
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
fn verification_reads_rhf_uhf_and_legacy_npy_representations() {
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
                assert_verification(&output, 0, "Verification successful", [2, 0, 0]);
                assert_eq!(output.stderr, b"");
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
fn verification_reads_legacy_inline_hf() {
    use rustiq_core::persistence::ConvergedHfArtifact;
    let directory = fixture();
    let root = directory.path();
    hf_bundle(root, false);
    let data = RustiQData::open(root.join("hf.rustiq")).unwrap();
    let hf = data.get::<ConvergedHfArtifact>().unwrap().unwrap();
    let energies = hf.energies();
    let convergence = hf.convergence();
    // Reconstruct the former inline JSON layout from public read-only accessors.
    let bytes = serde_json::to_vec(&json!({
        "version": hf.version(), "identity": hf.identity(),
        "basis_dimension": hf.basis_dimension(), "effective_rank": hf.effective_rank(),
        "iterations": hf.iterations(), "electronic_energy": energies[0],
        "nuclear_repulsion_energy": energies[1], "total_energy": energies[2],
        "delta_energy": convergence[0], "residual_norm": convergence[1],
        "linear_dependency_threshold": hf.linear_dependency_threshold(),
        "spin": hf.spin(), "orbitals": hf.orbitals()
    }))
    .unwrap();
    drop(data);
    edit_bundle(&root.join("hf.rustiq"), |manifest, members| {
        let artifact = &mut manifest["calculations"][0]["artifacts"]["converged_hf"];
        let path = artifact["path"].as_str().unwrap();
        members.iter_mut().find(|(name, _)| name == path).unwrap().1 = bytes.clone();
        artifact["representation"] = json!("rustiq-converged-hf-v1");
        artifact["size"] = json!(bytes.len());
        artifact["digest"] = json!(rustiq_core::persistence::sha256(&bytes));
    });
    let output = command(root, &["artifact", "verify", "hf.rustiq", "--strict"]);
    assert_verification(&output, 0, "Verification successful", [2, 0, 0]);
    assert_eq!(output.stderr, b"");
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
