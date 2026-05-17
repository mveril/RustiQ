use std::{env, fs, process::Command};

#[test]
fn test_cli_h2_sample_converges_and_prints_reference_energy() {
    let temp_root = env::temp_dir().join(format!("rustiq-cli-sample-test-{}", std::process::id()));
    let basis_dir = temp_root.join("RustiQ").join("basis_sets");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(&basis_dir).unwrap();
    fs::copy("tests/data/sto-3g.json", basis_dir.join("sto-3g.json")).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_RustiQ"))
        .args(["run", "--file", "samples/h2/calculation.toml"])
        .env("XDG_DATA_HOME", &temp_root)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "CLI failed with stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("SCF converged after 2 iterations."));
    assert!(stdout.contains("Total Energy (including nuclear repulsion): -1.116759 Hartree"));
}
