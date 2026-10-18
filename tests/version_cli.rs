#![allow(
    clippy::unwrap_used,
    reason = "Tests fail on unexpected process or UTF-8 errors"
)]

use std::{fs, process::Command};

fn version(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rustiq"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn short_version_is_unchanged() {
    for flag in ["--version", "-V"] {
        let output = version(&[flag]);
        assert!(output.status.success());
        assert_eq!(output.stderr, b"");
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!("rustiq {}\n", env!("CARGO_PKG_VERSION"))
        );
    }
}

#[test]
fn verbose_version_accepts_both_orders_and_short_flags() {
    let expected = version(&["--version", "--verbose"]);
    assert!(expected.status.success());
    assert_eq!(expected.stderr, b"");
    for args in [["--verbose", "--version"], ["-V", "-v"], ["-v", "-V"]] {
        let output = version(&args);
        assert!(output.status.success());
        assert_eq!(output.stdout, expected.stdout);
        assert_eq!(output.stderr, b"");
    }
    let text = String::from_utf8(expected.stdout).unwrap();
    assert!(!text.contains('\u{1b}'));
    let lines: Vec<_> = text.lines().collect();
    assert!(lines[1].starts_with("Git commit: "));
    for label in [
        "Target",
        "Rust compiler",
        "Rust compiler commit",
        "Optimization level",
        "Debug information",
        "Features",
    ] {
        assert!(lines
            .iter()
            .any(|line| line.starts_with(&format!("{label}: "))));
    }
    let feature_line = lines.last().unwrap();
    assert_eq!(feature_line.contains("online"), cfg!(feature = "online"));
    assert_eq!(
        feature_line.contains("bench-support"),
        cfg!(feature = "bench-support")
    );
}

#[test]
fn invalid_version_combinations_fail() {
    for args in [
        vec!["--verbose"],
        vec!["--version", "--invalid-option"],
        vec!["--version", "basis", "list"],
        vec!["--color", "never"],
    ] {
        assert_eq!(version(&args).status.code(), Some(2));
    }
}

#[test]
fn basis_list_verbose_keeps_its_existing_meaning() {
    let directory = tempfile::tempdir().unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_rustiq"))
            .args(args)
            .env("RUSTIQ_DATA_BASIS", directory.path())
            .env("RUSTIQ_COLOR", "never")
            .output()
            .unwrap()
    };
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/sto-3g.json");
    assert!(run(&["basis", "import", fixture]).status.success());
    let output = run(&["basis", "list", "--verbose"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8(output.stdout).unwrap().contains("STO-3G"));
}

#[test]
fn moved_binary_reports_embedded_metadata_without_build_tools_or_git() {
    let directory = tempfile::tempdir().unwrap();
    let binary = directory
        .path()
        .join(format!("rustiq{}", std::env::consts::EXE_SUFFIX));
    fs::copy(env!("CARGO_BIN_EXE_rustiq"), &binary).unwrap();
    let output = Command::new(binary)
        .args(["--version", "--verbose"])
        .current_dir(directory.path())
        .env("PATH", "")
        .env(
            "RUSTIQ_BUILD_GIT_COMMIT",
            "runtime values must not affect provenance",
        )
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, version(&["--version", "--verbose"]).stdout);
}
