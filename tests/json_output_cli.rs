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
    let data_home = TempDir::new().expect("temporary data home");
    let basis_store = data_home.path().join("RustiQ/basis_sets");
    fs::create_dir_all(&basis_store).expect("basis store directory");
    fs::copy(
        repo_root().join("tests/data/sto-3g.json"),
        basis_store.join("sto-3g.json"),
    )
    .expect("copy STO-3G basis fixture");

    let mut command = Command::new(env!("CARGO_BIN_EXE_RustiQ"));
    command
        .current_dir(repo_root())
        .env("RUSTIQ_DATA_HOME", data_home.path())
        .env("RUSTIQ_AUTO_DOWNLOAD", "0")
        .args(["run", sample]);
    if let Some(format) = format {
        command.args(["--format", format]);
    }
    command.output().expect("run RustiQ")
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

fn assert_v1_shape(output: &serde_json::Value) {
    let schema: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/schemas/calculation-output-v1.schema.json"
    )))
    .expect("valid calculation output schema");
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
