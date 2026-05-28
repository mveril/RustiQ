use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_root(test_name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    env::temp_dir().join(format!(
        "rustiq-{test_name}-{}-{unique}",
        std::process::id()
    ))
}

fn prepare_basis_store(temp_root: &Path) {
    let basis_dir = temp_root.join("RustiQ").join("basis_sets");
    let _ = fs::remove_dir_all(temp_root);
    fs::create_dir_all(&basis_dir).unwrap();
    fs::copy("tests/data/sto-3g.json", basis_dir.join("sto-3g.json")).unwrap();
}

fn run_rustiq(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_RustiQ"))
        .args(args)
        .output()
        .unwrap()
}

fn run_rustiq_with_data_home(args: &[&str], data_home: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_RustiQ"))
        .args(args)
        .env("RUSTIQ_DATA_HOME", data_home)
        .output()
        .unwrap()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "CLI failed with stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[cfg(feature = "online")]
fn test_online_basis_commands_are_available_with_default_features() {
    let basis_help = Command::new(env!("CARGO_BIN_EXE_RustiQ"))
        .args(["basis", "--help"])
        .output()
        .unwrap();
    assert!(basis_help.status.success());
    assert!(
        String::from_utf8_lossy(&basis_help.stdout).contains("download"),
        "basis help should expose download when the online feature is enabled"
    );

    let list_help = Command::new(env!("CARGO_BIN_EXE_RustiQ"))
        .args(["basis", "list", "--help"])
        .output()
        .unwrap();
    assert!(list_help.status.success());
    assert!(
        String::from_utf8_lossy(&list_help.stdout).contains("--online"),
        "basis list help should expose --online when the online feature is enabled"
    );
}

#[test]
#[cfg(not(feature = "online"))]
fn test_online_basis_commands_are_hidden_without_online_feature() {
    let basis_help = Command::new(env!("CARGO_BIN_EXE_RustiQ"))
        .args(["basis", "--help"])
        .output()
        .unwrap();
    assert!(basis_help.status.success());
    assert!(
        !String::from_utf8_lossy(&basis_help.stdout).contains("download"),
        "basis help should hide download when the online feature is disabled"
    );

    let list_help = Command::new(env!("CARGO_BIN_EXE_RustiQ"))
        .args(["basis", "list", "--help"])
        .output()
        .unwrap();
    assert!(list_help.status.success());
    assert!(
        !String::from_utf8_lossy(&list_help.stdout).contains("--online"),
        "basis list help should hide --online when the online feature is disabled"
    );
}

#[test]
fn test_cli_h2_sample_converges_and_prints_reference_energy() {
    let temp_root = temp_root("cli-sample");
    prepare_basis_store(&temp_root);

    let output = run_rustiq_with_data_home(
        &["run", "--file", "samples/h2/sto-3g/calculation.toml"],
        &temp_root,
    );

    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("SCF converged after 2 iterations."));
    assert!(stdout.contains("Total Energy (including nuclear repulsion): -1.116759 Hartree"));
}

#[test]
fn test_cli_h2_sample_can_disable_hf_formatting() {
    let temp_root = temp_root("cli-sample-no-format");
    prepare_basis_store(&temp_root);

    let toml_path = temp_root.join("calculation.toml");
    let molecule_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("samples")
        .join("h2")
        .join("molecule.xyz")
        .to_string_lossy()
        .replace('\\', "/");
    fs::write(
        &toml_path,
        format!(
            r#"
[global]
basis = "sto-3g"

[global.molecule]
geometry = "{molecule_path}"

[hf]
format = "Nope"
"#
        ),
    )
    .unwrap();

    let output =
        run_rustiq_with_data_home(&["run", "--file", toml_path.to_str().unwrap()], &temp_root);

    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("SCF converged after"));
    assert!(!stdout.contains("Total Energy (including nuclear repulsion):"));
}

#[test]
fn test_basis_remove_ignores_missing_names() {
    let temp_root = temp_root("basis-remove-missing");
    prepare_basis_store(&temp_root);

    let output = run_rustiq_with_data_home(&["basis", "remove", "sto-3g", "missing"], &temp_root);

    assert_success(&output);
    assert!(!temp_root
        .join("RustiQ")
        .join("basis_sets")
        .join("sto-3g.json")
        .exists());
}

#[test]
fn test_geometry_help_describes_transform_commands() {
    let output = run_rustiq(&["geometry", "--help"]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Inspect and transform molecular geometry files"));
    assert!(stdout.contains("rotate"));
    assert!(stdout.contains("translate"));
    assert!(stdout.contains("center"));
    assert!(stdout.contains("isometry"));
}

#[test]
fn test_geometry_rotate_help_describes_axis_format() {
    let output = run_rustiq(&["geometry", "rotate", "--help"]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("<ANGLE> <AXIS>"));
    assert!(stdout.contains("Rotation angle in degrees"));
    assert!(stdout.contains("Rotation axis: x, y, z"));
    assert!(stdout.contains("--angle <ANGLE>"));
    assert!(stdout.contains("--axis <AXIS>"));
}

#[test]
fn test_geometry_center_help_lists_center_types() {
    let output = run_rustiq(&["geometry", "center", "--help"]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("geometry center [OPTIONS] <CENTER_TYPE>"));
    assert!(stdout.contains("--center <CENTER_TYPE>"));
    assert!(stdout.contains("Possible values:"));
    assert!(stdout.contains("mass"));
    assert!(stdout.contains("geometry"));
    assert!(stdout.contains("charge"));
}

#[test]
fn test_geometry_translate_writes_valid_xyz_to_stdout() {
    let output = run_rustiq(&[
        "geometry",
        "translate",
        "--dx",
        "1",
        "samples/h2/molecule.xyz",
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("2\nHydrogen molecule"));
    assert!(stdout.contains("H      1.000000     0.000000    -0.370000"));
    assert!(stdout.contains("H      1.000000     0.000000     0.370000"));
}

#[test]
fn test_geometry_translate_writes_output_file() {
    let temp_root = temp_root("geometry-translate-output");
    let output_path = temp_root.join("translated.xyz");
    fs::create_dir_all(&temp_root).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_RustiQ"))
        .args([
            "geometry",
            "translate",
            "--dy",
            "1",
            "--output",
            output_path.to_str().unwrap(),
            "samples/h2/molecule.xyz",
        ])
        .output()
        .unwrap();
    assert_success(&output);

    let contents = fs::read_to_string(output_path).unwrap();
    assert!(contents.contains("H      0.000000     1.000000    -0.370000"));
    assert!(contents.contains("H      0.000000     1.000000     0.370000"));
}

#[test]
fn test_geometry_rotate_rotates_coordinates() {
    let output = run_rustiq(&[
        "geometry",
        "rotate",
        "--angle",
        "180",
        "--axis",
        "y",
        "samples/h2/molecule.xyz",
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("H     -0.000000     0.000000     0.370000"));
    assert!(stdout.contains("H      0.000000     0.000000    -0.370000"));
}

#[test]
fn test_geometry_center_supports_geometric_centering() {
    let temp_root = temp_root("geometry-center");
    let input_path = temp_root.join("linear.xyz");
    fs::create_dir_all(&temp_root).unwrap();
    fs::write(&input_path, "2\nLinear\nH 0.0 0.0 0.0\nHe 2.0 0.0 0.0\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_RustiQ"))
        .args([
            "geometry",
            "center",
            "--center",
            "geometry",
            input_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("H     -1.000000     0.000000     0.000000"));
    assert!(stdout.contains("He     1.000000     0.000000     0.000000"));
}

#[test]
fn test_geometry_isometry_applies_rotation_and_translation() {
    let temp_root = temp_root("geometry-isometry");
    let input_path = temp_root.join("point.xyz");
    fs::create_dir_all(&temp_root).unwrap();
    fs::write(&input_path, "1\nPoint\nH 1.0 0.0 0.0\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_RustiQ"))
        .args([
            "geometry",
            "isometry",
            "--dy",
            "1",
            "--angle",
            "90",
            "--axis",
            "z",
            input_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("H      0.000000     2.000000     0.000000"));
}
