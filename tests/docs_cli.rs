#![allow(
    clippy::unwrap_used,
    reason = "Tests fail on unexpected process or filesystem errors"
)]

use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn install(root: &Path, nix: bool) -> std::path::PathBuf {
    let directory = if nix {
        root.join("bin")
    } else {
        root.to_path_buf()
    };
    fs::create_dir_all(&directory).unwrap();
    let binary = directory.join(if cfg!(windows) {
        "rustiq.exe"
    } else {
        "rustiq"
    });
    fs::copy(env!("CARGO_BIN_EXE_rustiq"), &binary).unwrap();
    binary
}

fn docs(binary: &Path, args: &[&str], cwd: &Path) -> Output {
    Command::new(binary)
        .arg("docs")
        .args(args)
        .current_dir(cwd)
        .env("RUSTIQ_COLOR", "never")
        .env("BROWSER", cwd.join("nonexistent-browser"))
        .output()
        .unwrap()
}

#[test]
fn relocated_archive_and_nix_layout_resolve_without_the_working_directory() {
    for nix in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let original = temp.path().join("original");
        let binary = install(&original, nix);
        let manual = original.join(if nix {
            "share/doc/rustiq/manual/html"
        } else {
            "manual/html"
        });
        fs::create_dir_all(&manual).unwrap();
        fs::write(manual.join("index.html"), "<h1>Manual</h1>").unwrap();
        let moved = temp.path().join("moved installation with spaces");
        fs::rename(&original, &moved).unwrap();
        let binary = moved.join(binary.strip_prefix(&original).unwrap());
        let expected = moved
            .join(manual.strip_prefix(&original).unwrap())
            .join("index.html");
        let output = docs(&binary, &["--print-path"], temp.path());
        assert!(output.status.success(), "{output:?}");
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim(),
            expected.to_str().unwrap()
        );
        let web = docs(&binary, &["--web", "--print-path"], temp.path());
        assert!(web.status.success());
        assert_eq!(web.stdout, b"https://mveril.github.io/RustiQ/\n");
        let missing_browser = docs(&binary, &[], temp.path());
        assert!(!missing_browser.status.success());
        let error = String::from_utf8(missing_browser.stderr).unwrap();
        assert!(error.contains("--print-path"), "{error}");
        fs::remove_file(expected).unwrap();
        let broken = docs(&binary, &["--print-path"], temp.path());
        assert!(!broken.status.success());
        assert!(String::from_utf8(broken.stderr)
            .unwrap()
            .contains("incomplete"));
    }
}

#[test]
fn cargo_only_installation_reports_online_url_without_claiming_a_local_manual() {
    let temp = tempfile::tempdir().unwrap();
    let binary = install(&temp.path().join("cargo/bin"), false);
    let output = docs(&binary, &["--print-path"], temp.path());
    assert!(output.status.success());
    assert_eq!(output.stdout, b"https://mveril.github.io/RustiQ/\n");
    assert!(!docs(&binary, &[], temp.path()).status.success());
    assert!(!docs(&binary, &["--pdf"], temp.path()).status.success());
}

#[cfg(unix)]
#[test]
fn browser_receives_one_literal_target_and_failures_are_reported() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let binary = install(&temp.path().join("archive"), false);
    let manual = binary.parent().unwrap().join("manual/html");
    fs::create_dir_all(&manual).unwrap();
    fs::write(manual.join("index.html"), "Manual").unwrap();
    let browser = temp.path().join("test browser");
    fs::write(&browser, "#!/bin/sh\nprintf '%s\\n' \"$#\" \"$1\" > \"$BROWSER_LOG\"\nexit \"${BROWSER_STATUS:-0}\"\n").unwrap();
    fs::set_permissions(&browser, fs::Permissions::from_mode(0o755)).unwrap();
    let log = temp.path().join("browser.log");
    for (args, expected) in [
        (
            vec![],
            manual.join("index.html").to_str().unwrap().to_owned(),
        ),
        (vec!["--web"], "https://mveril.github.io/RustiQ/".to_owned()),
    ] {
        let output = Command::new(&binary)
            .arg("docs")
            .args(args)
            .env("BROWSER", &browser)
            .env("BROWSER_LOG", &log)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert_eq!(
            fs::read_to_string(&log).unwrap(),
            format!("1\n{expected}\n")
        );
    }
    let failed = Command::new(binary)
        .arg("docs")
        .env("BROWSER", browser)
        .env("BROWSER_LOG", log)
        .env("BROWSER_STATUS", "1")
        .output()
        .unwrap();
    assert!(!failed.status.success());
    assert!(String::from_utf8(failed.stderr)
        .unwrap()
        .contains("--print-path"));
}
