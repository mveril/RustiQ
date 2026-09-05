use std::{fs, io::Cursor, process::Command};
use RustiQ::basis::BasisStore;

#[test]
fn init_generates_runnable_hf_and_mp2_calculations() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("h2.xyz"), "2\nH2\nH 0 0 0\nH 0 0 0.74\n").unwrap();
    let data = temp.path().join("data");
    let store = BasisStore::new(&data.join("RustiQ/basis_sets"));
    store
        .import_as("sto-3g", Cursor::new(include_bytes!("data/sto-3g.json")))
        .unwrap();
    for options in [vec![], vec!["--mp2", "--force"]] {
        let result = Command::new(env!("CARGO_BIN_EXE_RustiQ"))
            .current_dir(temp.path())
            .args(["init", "h2.xyz"])
            .args(&options)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let result = Command::new(env!("CARGO_BIN_EXE_RustiQ"))
            .current_dir(temp.path())
            .env("RUSTIQ_DATA_HOME", &data)
            .args([
                "run",
                "calculation.toml",
                "--no-auto-download",
                "--format",
                "json",
            ])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let json: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(json["calculation"]["hf"]["converged"], true);
        assert_eq!(json["calculation"]["hf"]["method"], "RHF");
        assert_eq!(
            json["calculation"].get("mp2").is_some(),
            !options.is_empty()
        );
    }
}
