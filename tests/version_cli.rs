#![allow(
    clippy::unwrap_used,
    reason = "Tests fail on unexpected process or UTF-8 errors"
)]

use std::{fs, process::Command};

fn version(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rustiq"))
        .args(args)
        .env_remove("FORCE_HYPERLINK")
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
    assert!(lines.contains(&"SOURCE"));
    assert!(lines.contains(&"COMPILER"));
    assert!(lines.contains(&"CPU COMPATIBILITY"));
    assert!(lines.contains(&"BUILD CONFIGURATION"));
    assert!(!text.contains("PROFILING"));
    assert!(!text.contains("PLATFORM"));
    for label in [
        "Git commit",
        "Status",
        "Source SHA-256",
        "Cargo.lock SHA-256",
        "Rust",
        "LLVM",
        "Target",
        "Architecture",
        "Target features",
        "Optimization",
    ] {
        assert!(lines
            .iter()
            .any(|line| line.trim_start().starts_with(label)));
    }
    let architecture = lines
        .iter()
        .find(|line| line.starts_with("  Architecture"))
        .unwrap();
    assert!(architecture.contains(std::env::consts::ARCH));
    let feature_line = lines
        .iter()
        .find(|line| line.starts_with("  Cargo features"));
    let expected_any_feature = cfg!(feature = "online") || cfg!(feature = "bench-support");
    assert_eq!(feature_line.is_some(), expected_any_feature);
    if let Some(feature_line) = feature_line {
        assert!(!feature_line.contains("default"));
        assert_eq!(feature_line.contains("online"), cfg!(feature = "online"));
        assert_eq!(
            feature_line.contains("bench-support"),
            cfg!(feature = "bench-support")
        );
    }
    if let Some(debug_line) = lines
        .iter()
        .find(|line| line.starts_with("  Debug information"))
    {
        assert!(debug_line.ends_with("(declared)"));
    }
    assert!(!text.contains("Panic strategy"));
    assert!(!text.contains("Frame pointers"));
}

#[test]
fn invalid_version_combinations_fail() {
    for args in [
        vec!["--verbose"],
        vec!["--version", "--invalid-option"],
        vec!["--version", "build-info"],
        vec!["--version", "basis", "list"],
        vec!["build-info"],
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
        .env_remove("FORCE_HYPERLINK")
        .env(
            "RUSTIQ_BUILD_GIT_COMMIT",
            "runtime values must not affect provenance",
        )
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, version(&["--version", "--verbose"]).stdout);
}

#[test]
fn forced_hyperlinks_do_not_force_ansi_styling() {
    let output = Command::new(env!("CARGO_BIN_EXE_rustiq"))
        .args(["--version", "--verbose", "--color", "never"])
        .env("FORCE_HYPERLINK", "1")
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("\u{1b}]8;;"));
    assert!(!text.contains("\u{1b}["));
}

#[test]
fn version_color_flags_control_styling_independently_of_hyperlinks() {
    let always = Command::new(env!("CARGO_BIN_EXE_rustiq"))
        .args(["--version", "--verbose", "--color", "always"])
        .env_remove("FORCE_HYPERLINK")
        .output()
        .unwrap();
    assert!(always.status.success());
    assert!(always.stdout.windows(2).any(|window| window == b"\x1b["));

    let never = Command::new(env!("CARGO_BIN_EXE_rustiq"))
        .args(["--version", "--verbose", "--color", "never"])
        .env("FORCE_HYPERLINK", "1")
        .output()
        .unwrap();
    assert!(never.status.success());
    assert!(!never.stdout.windows(2).any(|window| window == b"\x1b["));
    assert!(never.stdout.windows(3).any(|window| window == b"\x1b]8"));
}
