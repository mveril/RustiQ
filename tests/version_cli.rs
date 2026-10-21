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
    assert!(lines.contains(&"CPU TARGET"));
    assert!(lines.contains(&"BUILD CONFIGURATION"));
    assert!(lines.contains(&"NUMERICAL LIBRARIES"));
    assert!(lines.contains(&"CHEMICAL DATA"));

    for (label, value) in [
        ("Debug assertions", env!("RUSTIQ_DEBUG_ASSERTIONS_DECLARED")),
        (
            "Integer overflow checks",
            env!("RUSTIQ_OVERFLOW_CHECKS_DECLARED"),
        ),
    ] {
        let row = lines
            .iter()
            .find(|line| line.trim_start().starts_with(label));
        let row = row.unwrap();
        let expected = if !value.is_empty() {
            format!("{value} (declared)")
        } else if label == "Debug assertions" {
            cfg!(debug_assertions).to_string()
        } else {
            "unknown".to_owned()
        };
        assert!(row.ends_with(&expected));
    }
    assert!(!text.contains("PROFILING"));
    assert!(!text.contains("PLATFORM"));
    for label in [
        "Git commit",
        "Status",
        "Rust",
        "Channel",
        "LLVM",
        "Target",
        "Optimization",
    ] {
        assert!(lines
            .iter()
            .any(|line| line.trim_start().starts_with(label)));
    }
    let architecture = lines.iter().find(|line| line.starts_with("  Architecture"));
    assert_eq!(
        architecture.is_some(),
        !env!("RUSTIQ_TARGET_ARCH").is_empty()
    );
    if let Some(architecture) = architecture {
        assert!(architecture.contains(env!("RUSTIQ_TARGET_ARCH")));
    }
    assert_eq!(
        lines.iter().any(|line| line.starts_with("  Target CPU")),
        !env!("RUSTIQ_BUILD_TARGET_CPU").is_empty()
    );
    let has_target_features = env!("RUSTIQ_TARGET_FEATURES")
        .split(',')
        .any(|feature| !feature.is_empty() && feature != "crt-static");
    assert_eq!(
        lines
            .iter()
            .any(|line| line.starts_with("  Target features")),
        has_target_features
    );
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
    assert_eq!(
        text.contains("Rust commit"),
        env!("VERGEN_RUSTC_CHANNEL") == "nightly" && env!("VERGEN_RUSTC_COMMIT_HASH").len() == 40
    );
}

#[test]
fn numerical_backends_show_versions_without_provenance_annotations() {
    let output = version(&["--version", "--verbose"]);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<_> = text.lines().collect();
    for (label, name, version) in [
        (
            "Linear algebra",
            "nalgebra",
            env!("RUSTIQ_NALGEBRA_VERSION"),
        ),
        ("Parallelism", "rayon", env!("RUSTIQ_RAYON_VERSION")),
        (
            "Special functions",
            "special",
            env!("RUSTIQ_SPECIAL_VERSION"),
        ),
    ] {
        let row = lines
            .iter()
            .find(|line| line.trim_start().starts_with(label))
            .unwrap();
        let value = if version.is_empty() {
            format!("{name} (version unknown)")
        } else {
            format!("{name} {version}")
        };
        assert!(row.ends_with(&value));
    }
    assert!(!text.contains("(lockfile)"));
}

#[test]
fn chemical_data_reports_locked_mendeleev_and_codata_revision() {
    let output = version(&["--version", "--verbose"]);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!text.contains("\x1b]8;;"));

    let version = env!("RUSTIQ_MENDELEEV_VERSION");
    assert!(
        !version.is_empty(),
        "mendeleev must be resolved from Cargo.lock"
    );
    assert!(!text.contains("periodic_table"));
    let mendeleev = text
        .lines()
        .find(|line| line.trim_start().starts_with("Periodic table"))
        .unwrap();
    let value = if version.is_empty() {
        "mendeleev (version unknown)".to_owned()
    } else {
        format!("mendeleev {version}")
    };
    assert!(mendeleev.ends_with(&value));

    let codata = text
        .lines()
        .find(|line| line.trim_start().starts_with("Bohr radius"))
        .unwrap();
    assert!(codata.ends_with(rustiq_core::molecules::BOHR_RADIUS_CODATA_REVISION));
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
    let source = env!("CARGO_BIN_EXE_rustiq");
    fs::write(&binary, fs::read(source).unwrap()).unwrap();
    fs::set_permissions(&binary, fs::metadata(source).unwrap().permissions()).unwrap();
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
    for (name, version) in [
        ("nalgebra", env!("RUSTIQ_NALGEBRA_VERSION")),
        ("rayon", env!("RUSTIQ_RAYON_VERSION")),
        ("special", env!("RUSTIQ_SPECIAL_VERSION")),
        ("mendeleev", env!("RUSTIQ_MENDELEEV_VERSION")),
    ] {
        if !version.is_empty() {
            assert!(text.contains(&format!("https://crates.io/crates/{name}/{version}\x1b\\")));
        }
    }
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
