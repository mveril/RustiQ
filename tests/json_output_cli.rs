#![allow(
    clippy::unwrap_used,
    reason = "Integration tests and their fixture helpers intentionally panic on unexpected failures"
)]

use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};

use approx::assert_abs_diff_eq;
use tempfile::TempDir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run_command(sample: &str, format: Option<&str>) -> Output {
    run_command_with_color(sample, format, None)
}

fn run_command_with_color(sample: &str, format: Option<&str>, color: Option<&str>) -> Output {
    run_command_with_options(sample, format, color, false)
}

fn run_command_with_options(
    sample: &str,
    format: Option<&str>,
    color: Option<&str>,
    pretty: bool,
) -> Output {
    let data_home = TempDir::new().expect("temporary data home");
    let basis_store = data_home.path().join("rustiq/basis_sets");
    fs::create_dir_all(&basis_store).expect("basis store directory");
    fs::copy(
        repo_root().join("tests/data/sto-3g.json"),
        basis_store.join("sto-3g.json"),
    )
    .expect("copy STO-3G basis fixture");
    fs::copy(
        repo_root().join("tests/data/reference/RustiQ/basis_sets/6-31g.json"),
        basis_store.join("6-31g.json"),
    )
    .expect("copy 6-31G basis fixture");

    let mut command = Command::new(env!("CARGO_BIN_EXE_rustiq"));
    command
        .current_dir(repo_root())
        .env("RUSTIQ_DATA_HOME", data_home.path())
        .env("RUSTIQ_AUTO_DOWNLOAD", "0")
        .env_remove("NO_COLOR")
        .args(["run", sample]);
    if let Some(color) = color {
        command.env("RUSTIQ_COLOR", color);
    } else {
        command.env_remove("RUSTIQ_COLOR");
    }
    if let Some(format) = format {
        command.args(["--format", format]);
    }
    if pretty {
        command.arg("--pretty");
    }
    command.output().expect("run RustiQ")
}

#[test]
fn pretty_json_obeys_color_setting() {
    for pretty in [false, true] {
        for color in ["always", "never"] {
            let output = run_command_with_options(
                "samples/h2/sto-3g/calculation.toml",
                Some("json"),
                Some(color),
                pretty,
            );
            assert!(
                output.status.success(),
                "{color}, pretty={pretty}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert_eq!(stdout.contains("\x1b["), pretty && color == "always");
            if !pretty || color == "never" {
                let value: serde_json::Value = serde_json::from_slice(&output.stdout)
                    .expect("JSON stdout without syntax highlighting must remain valid");
                assert!(value["calculation"]["hf"].is_object());
                if pretty {
                    assert!(stdout.contains("\n  \"calculation\""));
                } else {
                    assert!(stdout
                        .starts_with("{\"schema_version\":1,\"calculation\":{\"hf\":{\"method\":"));
                    assert_eq!(stdout.bytes().filter(|byte| *byte == b'\n').count(), 1);
                }
            }
        }
    }
}

#[test]
fn pretty_batch_json_obeys_color_setting() {
    let sample = "samples/h2/study.ncl";
    let compact = run_command_with_options(sample, Some("json"), Some("never"), false);
    assert!(
        compact.status.success(),
        "{}",
        String::from_utf8_lossy(&compact.stderr)
    );
    let expected: serde_json::Value = serde_json::from_slice(&compact.stdout).unwrap();
    assert!(batch_validator().is_valid(&expected));

    for pretty in [false, true] {
        for color in ["always", "never"] {
            let output = run_command_with_options(sample, Some("json"), Some(color), pretty);
            assert!(
                output.status.success(),
                "{color}, pretty={pretty}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert_eq!(stdout.contains("\x1b["), pretty && color == "always");
            if !pretty || color == "never" {
                let value: serde_json::Value = serde_json::from_slice(&output.stdout)
                    .expect("batch JSON without highlighting must remain valid");
                assert_eq!(value, expected);
                assert!(batch_validator().is_valid(&value));
                if pretty {
                    assert!(stdout.contains("\n  \"calculations\": ["));
                    assert!(stdout.contains("\n    {"));
                } else {
                    assert_eq!(stdout.bytes().filter(|byte| *byte == b'\n').count(), 1);
                }
            }
        }
    }
}

#[test]
fn pretty_batch_preserves_outcomes_and_failure_status() {
    let directory = TempDir::new().unwrap();
    fs::copy(
        repo_root().join("samples/h2/molecule.xyz"),
        directory.path().join("molecule.xyz"),
    )
    .unwrap();
    let input = directory.path().join("mixed.ncl");
    fs::write(
        &input,
        r#"[
            { basis.name = "sto-3g", molecule.geometry = "molecule.xyz" },
            { basis.name = "sto-3g", molecule.geometry = "missing.xyz" },
            { basis.name = "sto-3g", molecule.geometry = "molecule.xyz" },
        ]"#,
    )
    .unwrap();
    let sample = input.to_str().unwrap();
    let compact = run_command_with_options(sample, Some("json"), Some("never"), false);
    let pretty = run_command_with_options(sample, Some("json"), Some("never"), true);
    assert!(!compact.status.success());
    assert!(!pretty.status.success());

    let compact_json: serde_json::Value = serde_json::from_slice(&compact.stdout).unwrap();
    let pretty_json: serde_json::Value = serde_json::from_slice(&pretty.stdout).unwrap();
    assert_eq!(pretty_json, compact_json);
    assert!(batch_validator().is_valid(&pretty_json));
    let entries = pretty_json["calculations"].as_array().unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0]["status"], "success");
    assert_eq!(entries[1]["status"], "error");
    assert_eq!(entries[2]["status"], "success");
    assert_eq!(entries[0]["result"], entries[2]["result"]);
    assert!(String::from_utf8_lossy(&pretty.stdout).contains("\n  \"calculations\": ["));
    assert!(String::from_utf8_lossy(&pretty.stderr).contains("batch contains 1 failed"));
}

#[test]
fn pretty_requires_json_format() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_rustiq"))
        .args([
            "run",
            "samples/h2/sto-3g/calculation.toml",
            "--pretty",
            "--format",
            "text",
        ])
        .output()
        .expect("run RustiQ");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--pretty requires --format json"));
}

#[test]
fn pretty_without_format_requires_json_format() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_rustiq"))
        .args(["run", "samples/h2/sto-3g/calculation.toml", "--pretty"])
        .output()
        .expect("run RustiQ");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--pretty requires --format json"));
}

fn json_output(sample: &str) -> serde_json::Value {
    let output = run_command(sample, Some("json"));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("JSON-only stdout")
}

fn batch_validator() -> jsonschema::Validator {
    let mut schema: serde_json::Value =
        serde_json::from_str(include_str!("../schemas/batch-output-v1.schema.json")).unwrap();
    let calculation_schema: serde_json::Value =
        serde_json::from_str(include_str!("../schemas/calculation-output-v1.schema.json")).unwrap();
    schema["$defs"] = serde_json::json!({ "calculation": calculation_schema });
    for outcome in schema["properties"]["calculations"]["items"]["oneOf"]
        .as_array_mut()
        .unwrap()
    {
        if outcome["properties"]["result"].is_object() {
            outcome["properties"]["result"]["$ref"] =
                serde_json::Value::String("#/$defs/calculation".to_owned());
        }
    }
    jsonschema::validator_for(&schema).expect("valid batch JSON Schema")
}

#[test]
fn nickel_sample_batch_preserves_individual_v1_results_and_batch_schema() {
    let value = json_output("samples/h2/study.ncl");
    assert!(
        batch_validator().is_valid(&value),
        "invalid batch JSON: {value}"
    );
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schemas/batch-output-v1.schema.json")).unwrap();
    assert_eq!(
        value["schema_version"],
        schema["properties"]["schema_version"]["const"]
    );
    assert_eq!(value["kind"], schema["properties"]["kind"]["const"]);
    assert_eq!(value.as_object().unwrap().len(), 3);
    let entries = value["calculations"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    let success_schema = &schema["properties"]["calculations"]["items"]["oneOf"][0];
    assert_eq!(
        success_schema["properties"]["result"]["$ref"],
        "calculation-output-v1.schema.json"
    );
    for entry in entries {
        assert_eq!(entry["status"], "success");
        assert_eq!(entry.as_object().unwrap().len(), 2);
        for field in success_schema["required"].as_array().unwrap() {
            assert!(entry.get(field.as_str().unwrap()).is_some());
        }
        assert_v1_shape(&entry["result"]);
    }
    for (index, basis) in ["sto-3g", "6-31g"].iter().enumerate() {
        let baseline = json_output(&format!("samples/h2/{basis}/calculation.toml"));
        let hf = &entries[index]["result"]["calculation"]["hf"];
        assert_abs_diff_eq!(
            hf["total_energy"].as_f64().unwrap(),
            baseline["calculation"]["hf"]["total_energy"]
                .as_f64()
                .unwrap(),
            epsilon = 1e-9
        );
        assert_eq!(
            hf["orthogonalization"]["ao_basis_dimension"],
            baseline["calculation"]["hf"]["orthogonalization"]["ao_basis_dimension"]
        );
    }
    let text = run_command("samples/h2/study.ncl", Some("text"));
    assert!(
        text.status.success(),
        "{}",
        String::from_utf8_lossy(&text.stderr)
    );
    let text = String::from_utf8(text.stdout).unwrap();
    assert!(text.contains("Calculation 1/2"));
    assert!(text.contains("Calculation 2/2"));
    assert!(text.contains("2 succeeded, 0 non-converged, 0 failed"));
}

#[test]
fn single_nickel_sample_runs_transparently_like_toml() {
    let native = json_output("samples/h2/sto-3g/calculation.ncl");
    let toml = json_output("samples/h2/sto-3g/calculation.toml");
    assert_v1_shape(&native);
    assert_eq!(native, toml);
    let output = run_command("samples/h2/sto-3g/calculation.ncl", None);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Requested calculation (canonical TOML)")
    );
}

#[test]
fn mp2_human_memory_budget_reaches_cli_and_preserves_json() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("memory.toml");
    let geometry = repo_root().join("samples/h2/molecule.xyz");
    let prefix = format!(
        "[molecule]\ngeometry = {:?}\n[basis]\nname = \"sto-3g\"\n[method.hf]\n[method.mp2]\n",
        geometry.to_str().unwrap()
    );
    fs::write(&path, format!("{prefix}memory_limit = \"1 KiB\"\n")).unwrap();
    let sample = path.to_str().unwrap();
    let output = json_output(sample);
    assert_v1_shape(&output);
    let forced_color_json = run_command_with_color(sample, Some("json"), Some("always"));
    assert!(forced_color_json.status.success());
    assert!(!String::from_utf8_lossy(&forced_color_json.stdout).contains("\x1b["));
    serde_json::from_slice::<serde_json::Value>(&forced_color_json.stdout)
        .expect("forced colors must not contaminate JSON stdout");
    let text = run_command(sample, Some("text"));
    assert!(
        text.status.success(),
        "{}",
        String::from_utf8_lossy(&text.stderr)
    );
    let text = String::from_utf8(text.stdout).unwrap();
    assert!(text.contains("budget 1.0 KiB"), "{text}");
    assert!(text.contains("block 1"), "{text}");
    for value in ["1 B", "0 MiB", "nonsense"] {
        fs::write(&path, format!("{prefix}memory_limit = {value:?}\n")).unwrap();
        let output = run_command(sample, Some("json"));
        assert!(!output.status.success(), "{value}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("memory_limit"), "{stderr}");
    }
}

fn assert_v1_shape(output: &serde_json::Value) {
    let schema: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/schemas/calculation-output-v1.schema.json"
    )))
    .expect("valid calculation output schema");
    let validator = jsonschema::validator_for(&schema).expect("valid calculation JSON Schema");
    assert!(
        validator.is_valid(output),
        "invalid calculation JSON: {output}"
    );
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(
        output["schema_version"],
        schema["properties"]["schema_version"]["const"]
    );
    let root = output.as_object().expect("output object");
    assert_eq!(root.len(), 2);
    let calculation = output["calculation"]
        .as_object()
        .expect("calculation object");
    assert!(calculation.len() == 1 || calculation.len() == 2);
    let hf = calculation["hf"].as_object().expect("HF object");
    for key in [
        "method",
        "converged",
        "iterations",
        "electronic_energy",
        "nuclear_repulsion_energy",
        "total_energy",
        "delta_energy",
        "residual_norm",
        "orthogonalization",
    ] {
        assert!(hf.contains_key(key), "missing HF field {key}");
    }
    assert!(hf["method"] == "RHF" || hf["method"] == "UHF");
    assert!(hf["converged"].is_boolean());
    assert!(hf["iterations"].is_u64());
    let orthogonalization = hf["orthogonalization"].as_object().unwrap();
    for key in [
        "ao_basis_dimension",
        "effective_rank",
        "discarded_directions",
        "relative_linear_dependency_threshold",
    ] {
        assert!(
            orthogonalization.contains_key(key),
            "missing orthogonalization field {key}"
        );
    }
    if hf["method"] == "UHF" {
        let spin = hf["spin"].as_object().expect("UHF spin object");
        for key in ["s_squared", "ideal_s_squared", "spin_contamination"] {
            assert!(spin[key].is_number(), "missing spin field {key}");
            assert!(schema["$defs"]["spin"]["properties"].get(key).is_some());
        }
    } else {
        assert!(hf.get("spin").is_none());
    }
    if let Some(mp2) = calculation.get("mp2") {
        let mp2 = mp2.as_object().expect("MP2 object");
        assert!(mp2["method"] == "RHF-MP2" || mp2["method"] == "UHF-MP2");
        for key in ["correlation_energy", "electronic_energy", "total_energy"] {
            assert!(mp2[key].is_number(), "missing MP2 field {key}");
        }
    }
}

#[test]
fn json_rhf_output_is_machine_readable_and_full_precision() {
    let output = json_output("samples/h2/sto-3g/calculation.toml");
    assert_v1_shape(&output);

    assert_eq!(output["schema_version"], 1);
    assert_eq!(output["calculation"]["hf"]["method"], "RHF");
    assert!(output["calculation"].get("mp2").is_none());
    assert_abs_diff_eq!(
        output["calculation"]["hf"]["total_energy"]
            .as_f64()
            .unwrap(),
        -1.116_759_307_506_361_3,
        epsilon = 1e-14
    );
}

#[test]
fn json_uhf_and_mp2_output_expose_structured_results() {
    let uhf = json_output("samples/h2/sto-3g/uhf_h2_plus_calculation.toml");
    assert_v1_shape(&uhf);
    assert_eq!(uhf["calculation"]["hf"]["method"], "UHF");
    let spin = &uhf["calculation"]["hf"]["spin"];
    assert_abs_diff_eq!(spin["s_squared"].as_f64().unwrap(), 0.75, epsilon = 1e-10);
    assert_abs_diff_eq!(
        spin["ideal_s_squared"].as_f64().unwrap(),
        0.75,
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        spin["spin_contamination"].as_f64().unwrap(),
        0.0,
        epsilon = 1e-10
    );

    let mp2 = json_output("samples/h2/sto-3g/mp2_calculation.toml");
    assert_v1_shape(&mp2);
    assert_eq!(mp2["calculation"]["mp2"]["method"], "RHF-MP2");
    assert_abs_diff_eq!(
        mp2["calculation"]["mp2"]["correlation_energy"]
            .as_f64()
            .unwrap(),
        -0.013_138_073_583_781_103,
        epsilon = 1e-14
    );
}

#[test]
fn normal_output_remains_human_readable() {
    let output = run_command("samples/h2/sto-3g/calculation.toml", None);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 report");
    assert!(stdout.contains("Total Energy (including nuclear repulsion): -1.116759 Hartree"));
}
