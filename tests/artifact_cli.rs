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
    let store = root.join("data/RustiQ/basis_sets");
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
    Command::new(env!("CARGO_BIN_EXE_RustiQ"))
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
    let mut restored = RustiQData::open(root.join("state.rustiq")).unwrap();
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
    let mut bundle = RustiQBundle::open(root.join("state.rustiq")).unwrap();
    assert_eq!(bundle.calculations().len(), 2);
    for data in bundle.calculations_mut() {
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
    assert!(!output.status.success());
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
    assert!(!output.status.success());
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
