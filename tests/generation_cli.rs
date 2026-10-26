#![allow(
    clippy::unwrap_used,
    reason = "Tests fail on unexpected process or filesystem errors"
)]

use std::process::{Command, Output};

fn rustiq(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rustiq"))
        .args(args)
        .env("RUSTIQ_COLOR", "never")
        .output()
        .unwrap()
}

#[test]
fn generator_availability_matches_features() {
    let output = rustiq(&["--help"]);
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    for (name, enabled) in [
        ("manpages", cfg!(feature = "manpages")),
        ("completions", cfg!(feature = "completions")),
    ] {
        assert_eq!(help.contains(name), enabled);
        assert_eq!(rustiq(&[name, "--help"]).status.success(), enabled);
    }
}

#[cfg(feature = "completions")]
#[test]
fn all_shells_describe_the_application_without_generators() {
    for shell in ["bash", "zsh", "fish", "elvish", "powershell", "nushell"] {
        let output = rustiq(&["completions", shell]);
        assert!(output.status.success(), "{shell}: {output:?}");
        assert!(output.stderr.is_empty(), "{shell}: {output:?}");
        let script = String::from_utf8(output.stdout).unwrap();
        for expected in ["rustiq", "artifact", "inspect", "color"] {
            assert!(script.contains(expected), "{shell}: missing {expected}");
        }
        assert!(!script.contains("manpages"), "{shell}");
        assert!(
            !script.contains("Write a shell completion script"),
            "{shell}"
        );
        assert!(!script.contains('\u{1b}'), "{shell}");
    }
}

#[cfg(feature = "completions")]
#[test]
fn invalid_shell_is_rejected_without_a_script() {
    let output = rustiq(&["completions", "invalid-shell"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stdout, b"");
    assert_ne!(output.stderr, b"");
}

#[cfg(feature = "completions")]
#[test]
fn omitted_shell_uses_environment_fallback_and_explicit_shell_takes_precedence() {
    let expected = rustiq(&["completions", "fish"]);
    assert!(expected.status.success());
    let detected = Command::new(env!("CARGO_BIN_EXE_rustiq"))
        .arg("completions")
        .env("SHELL", "/usr/bin/fish")
        .output()
        .unwrap();
    assert!(detected.status.success(), "{detected:?}");
    assert_eq!(detected.stdout, expected.stdout);
    let explicit = Command::new(env!("CARGO_BIN_EXE_rustiq"))
        .args(["completions", "fish"])
        .env("SHELL", "/unsupported-shell")
        .output()
        .unwrap();
    assert!(explicit.status.success());
    assert_eq!(explicit.stdout, expected.stdout);
}

#[cfg(feature = "completions")]
#[test]
fn unknown_shell_requests_an_explicit_choice() {
    let output = Command::new(env!("CARGO_BIN_EXE_rustiq"))
        .arg("completions")
        .env("SHELL", "/unsupported-shell")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("could not detect a supported shell"));
}

#[cfg(all(feature = "completions", unix))]
#[test]
fn calling_shell_takes_precedence_over_login_shell() {
    let output = Command::new("bash")
        .args([
            "-c",
            "\"$1\" completions; status=$?; exit \"$status\"",
            "_",
            env!("CARGO_BIN_EXE_rustiq"),
        ])
        .env("SHELL", "/usr/bin/fish")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, rustiq(&["completions", "bash"]).stdout);
}

#[cfg(feature = "manpages")]
#[test]
fn manpages_include_nested_commands_global_options_and_book_link() {
    let temp = tempfile::tempdir().unwrap();
    let destination = temp.path().join("new").join("manpages");
    let output = Command::new(env!("CARGO_BIN_EXE_rustiq"))
        .arg("manpages")
        .arg(&destination)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"");
    assert_eq!(output.stderr, b"");
    for name in ["rustiq.1", "rustiq-run.1", "rustiq-artifact-inspect.1"] {
        let page = std::fs::read_to_string(destination.join(name)).unwrap();
        assert!(page.contains("SEE ALSO"), "{name}");
        assert!(page.contains("https://mveril.github.io/RustiQ/"), "{name}");
        assert!(page.contains("color"), "{name}");
        assert!(!page.contains("manpages"), "{name}");
        assert!(!page.contains("completions"), "{name}");
    }
    assert!(!destination.join("rustiq-manpages.1").exists());
    assert!(!destination.join("rustiq-completions.1").exists());
}

#[cfg(feature = "manpages")]
#[test]
fn manpages_report_filesystem_errors() {
    let temp = tempfile::tempdir().unwrap();
    let destination = temp.path().join("file");
    std::fs::write(&destination, b"preserve me").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rustiq"))
        .arg("manpages")
        .arg(&destination)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_ne!(output.stderr, b"");
    assert_eq!(std::fs::read(destination).unwrap(), b"preserve me");
}

#[cfg(all(feature = "completions", target_os = "linux"))]
#[test]
fn completion_write_errors_are_reported() {
    let output = Command::new(env!("CARGO_BIN_EXE_rustiq"))
        .args(["completions", "nushell"])
        .stdout(
            std::fs::File::options()
                .write(true)
                .open("/dev/full")
                .unwrap(),
        )
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_ne!(output.stderr, b"");
}
