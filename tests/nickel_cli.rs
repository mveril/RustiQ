#![allow(
    clippy::unwrap_used,
    reason = "Tests fail immediately on unexpected fixture errors"
)]

use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use tempfile::TempDir;

fn run(directory: &Path, input: &Path, format: &str) -> Output {
    let store = directory.join("data/RustiQ/basis_sets");
    fs::create_dir_all(&store).unwrap();
    fs::write(
        store.join("sto-3g.json"),
        include_bytes!("data/sto-3g.json"),
    )
    .unwrap();
    Command::new(env!("CARGO_BIN_EXE_RustiQ"))
        .current_dir(directory)
        .env("RUSTIQ_DATA_HOME", directory.join("data"))
        .env("RUSTIQ_AUTO_DOWNLOAD", "0")
        .args(["--color", "always", "run"])
        .arg(input)
        .args(["--format", format])
        .output()
        .unwrap()
}

fn fixture() -> TempDir {
    let directory = tempfile::tempdir().unwrap();
    fs::create_dir_all(directory.path().join("inputs/nested")).unwrap();
    fs::write(
        directory.path().join("inputs/molecule.xyz"),
        "2\nH2\nH 0 0 0\nH 0 0 0.74\n",
    )
    .unwrap();
    directory
}

#[test]
fn native_imports_and_resources_work_from_another_directory() {
    let directory = fixture();
    let root = directory.path();
    fs::write(
        root.join("inputs/nested/base.toml"),
        "[basis]\nname = 'sto-3g'\n",
    )
    .unwrap();
    fs::write(
        root.join("inputs/common.ncl"),
        "import \"nested/base.toml\"",
    )
    .unwrap();
    let input = root.join("inputs/calculation.NCL");
    fs::write(&input, "import \"common.ncl\"").unwrap();
    let output = run(root, &input, "json");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let native: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(native["schema_version"], 1);
    assert_eq!(native["calculation"]["hf"]["converged"], true);
    let toml = root.join("inputs/calculation.toml");
    fs::write(&toml, "[basis]\nname = 'sto-3g'\n").unwrap();
    let output = run(root, &toml, "json");
    let baseline: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(native, baseline);
    let relative = run(root, Path::new("inputs/calculation.NCL"), "json");
    assert!(
        relative.status.success(),
        "{}",
        String::from_utf8_lossy(&relative.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&relative.stdout).unwrap(),
        native
    );
    fs::write(&input, "[import \"common.ncl\"]").unwrap();
    let output = run(root, &input, "json");
    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
        native
    );
}

#[test]
fn batch_continues_after_error_and_preserves_order_and_json() {
    let directory = fixture();
    let input = directory.path().join("inputs/batch.ncl");
    fs::write(&input, "std.array.map (fun geometry => { basis.name = \"sto-3g\", molecule.geometry = geometry }) [\"molecule.xyz\", \"missing.xyz\", \"molecule.xyz\"]").unwrap();
    let output = run(directory.path(), &input, "json");
    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["kind"], "batch");
    let entries = value["calculations"].as_array().unwrap();
    assert_eq!(entries.len(), 3);
    for (index, entry) in entries.iter().enumerate() {
        assert_eq!(entry["index"], index);
    }
    assert_eq!(entries[0]["status"], "success");
    assert_eq!(entries[1]["status"], "error");
    assert!(entries[1]["error"]["message"].is_string());
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schemas/batch-output-v1.schema.json")).unwrap();
    let error_schema = &schema["properties"]["calculations"]["items"]["oneOf"][1];
    assert_eq!(
        entries[1]["status"],
        error_schema["properties"]["status"]["const"]
    );
    for field in error_schema["required"].as_array().unwrap() {
        assert!(entries[1].get(field.as_str().unwrap()).is_some());
    }
    assert_eq!(entries[1].as_object().unwrap().len(), 3);
    assert_eq!(entries[1]["error"].as_object().unwrap().len(), 1);
    assert_eq!(entries[2]["status"], "success");
    assert_eq!(entries[0]["result"], entries[2]["result"]);
    let text = run(directory.path(), &input, "text");
    let text = String::from_utf8(text.stdout).unwrap();
    assert!(text.contains("Calculation 3/3"));
    assert!(text.contains("2 succeeded, 0 non-converged, 1 failed"));
}

#[test]
fn configuration_errors_abort_before_execution_and_keep_import_sources() {
    let directory = fixture();
    let input = directory.path().join("inputs/error.ncl");
    for source in ["[]", "[{ basis.name = \"sto-3g\" }, { basis.name = 42 }]"] {
        fs::write(&input, source).unwrap();
        let output = run(directory.path(), &input, "json");
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
    let bad = directory.path().join("inputs/nested/bad.ncl");
    for source in ["{ basis.name = 42 }", "{ basis.name = "] {
        fs::write(&bad, source).unwrap();
        fs::write(&input, "import \"nested/bad.ncl\"").unwrap();
        let output = run(directory.path(), &input, "json");
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("bad.ncl"));
    }
    fs::write(&input, "import \"absent.ncl\"").unwrap();
    let output = run(directory.path(), &input, "json");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("absent.ncl"));
}

#[test]
fn batch_reports_non_convergence_and_mp2_error_then_continues() {
    let directory = fixture();
    let input = directory.path().join("inputs/scf.ncl");
    fs::write(&input, "[ { basis.name = \"sto-3g\", method.hf.max_iterations = 1 }, { basis.name = \"sto-3g\", method.hf.max_iterations = 1, method.mp2 = {} }, { basis.name = \"sto-3g\" } ]").unwrap();
    let output = run(directory.path(), &input, "json");
    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["calculations"][0]["status"], "non_converged");
    assert_eq!(
        value["calculations"][0]["result"]["calculation"]["hf"]["converged"],
        false
    );
    assert_eq!(value["calculations"][1]["status"], "error");
    assert_eq!(value["calculations"][2]["status"], "success");
}
