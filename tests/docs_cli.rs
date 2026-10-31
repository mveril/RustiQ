#![allow(
    clippy::unwrap_used,
    reason = "Tests fail on unexpected process or filesystem errors"
)]

use std::{
    fs, io,
    path::Path,
    process::{Command, Output},
    thread,
    time::Duration,
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

fn expected_online() -> String {
    if env!("RUSTIQ_DOCS_RELEASE") == "true" {
        format!(
            "https://mveril.github.io/RustiQ/v{}/",
            env!("CARGO_PKG_VERSION")
        )
    } else {
        "https://mveril.github.io/RustiQ/dev/".to_owned()
    }
}

fn docs(binary: &Path, args: &[&str], cwd: &Path) -> Output {
    let mut command = Command::new(binary);
    command
        .arg("docs")
        .args(args)
        .current_dir(cwd)
        .env("RUSTIQ_COLOR", "never")
        .env("BROWSER", cwd.join("nonexistent-browser"));
    // Keep host-installed openers such as xdg-open from masking a missing or
    // failing BROWSER command. The executable under test is invoked by path.
    #[cfg(unix)]
    command.env("PATH", cwd);
    run_output(&mut command)
}

fn run_output(command: &mut Command) -> Output {
    // Linux can briefly return ETXTBSY when a freshly installed executable is
    // still being finalized by the filesystem. Retry only that transient
    // process-start error; preserve all other failures for the test to report.
    let mut last_error = None;
    for _ in 0..40 {
        match command.output() {
            Ok(output) => return output,
            Err(error) if is_executable_busy(&error) => {
                last_error = Some(error);
                thread::sleep(Duration::from_millis(25));
            }
            Err(error) => return unwrap_output(Err(error)),
        }
    }
    unwrap_output(Err(
        last_error.expect("a busy executable error was observed")
    ))
}

fn unwrap_output(output: io::Result<Output>) -> Output {
    output.unwrap()
}

fn is_executable_busy(error: &io::Error) -> bool {
    #[cfg(target_os = "linux")]
    {
        error.raw_os_error() == Some(26) // ETXTBSY
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = error;
        false
    }
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
        assert_eq!(
            String::from_utf8(web.stdout).unwrap().trim(),
            expected_online()
        );
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let missing_browser = docs(&binary, &[], temp.path());
            assert!(!missing_browser.status.success());
            let error = String::from_utf8(missing_browser.stderr).unwrap();
            assert!(error.contains("--print-path"), "{error}");
        }
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
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        expected_online()
    );
    #[cfg(all(unix, not(target_os = "macos")))]
    assert!(!docs(&binary, &[], temp.path()).status.success());
    assert!(!docs(&binary, &["--pdf"], temp.path()).status.success());
}

#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn browser_receives_one_literal_target_and_failures_are_reported() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let binary = install(&temp.path().join("archive"), false);
    let manual = binary.parent().unwrap().join("manual/html");
    fs::create_dir_all(&manual).unwrap();
    fs::write(manual.join("index.html"), "Manual").unwrap();
    // webbrowser launches graphical browsers asynchronously. Naming this fake
    // launcher `lynx` makes it use the crate's synchronous text-browser path,
    // so the assertions can inspect its output as soon as `docs` exits.
    let browser = temp.path().join("lynx");
    fs::write(&browser, "#!/bin/sh\nprintf '%s\\n' \"$#\" \"$1\" > \"$BROWSER_LOG\"\nexit \"${BROWSER_STATUS:-0}\"\n").unwrap();
    fs::set_permissions(&browser, fs::Permissions::from_mode(0o755)).unwrap();
    let log = temp.path().join("browser.log");
    for (args, expected) in [
        (
            vec![],
            format!("file://{}", manual.join("index.html").display()),
        ),
        (vec!["--web"], expected_online()),
    ] {
        let mut command = Command::new(&binary);
        command
            .arg("docs")
            .args(args)
            .env("BROWSER", &browser)
            .env("BROWSER_LOG", &log)
            .env("PATH", temp.path());
        let output = run_output(&mut command);
        assert!(output.status.success(), "{output:?}");
        assert_eq!(
            fs::read_to_string(&log).unwrap(),
            format!("1\n{expected}\n")
        );
    }
    let mut command = Command::new(binary);
    command
        .arg("docs")
        .env("BROWSER", browser)
        .env("BROWSER_LOG", log)
        .env("BROWSER_STATUS", "1")
        .env("PATH", temp.path());
    let failed = run_output(&mut command);
    assert!(!failed.status.success());
    assert!(String::from_utf8(failed.stderr)
        .unwrap()
        .contains("--print-path"));
}

// Inputs are embedded at compile time; execution needs only the relocated binary
// and files created in this temporary directory, never a source checkout.
#[test]
fn standalone_manual_workflows_run_offline() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let binary = install(&root.join("installed"), false);
    let run = |args: &[&str]| {
        let mut command = Command::new(&binary);
        command
            .current_dir(root)
            .env("RUSTIQ_DATA_HOME", root.join("data"))
            .env_remove("RUSTIQ_DATA_BASIS")
            .env("RUSTIQ_CACHE_HOME", root.join("cache"))
            .env("RUSTIQ_AUTO_DOWNLOAD", "0")
            .env("RUSTIQ_COLOR", "never")
            .args(args);
        // Invoke by absolute path without external tools on Unix; Windows
        // retains the system PATH for native dependency resolution.
        #[cfg(unix)]
        command.env("PATH", root);
        let output = run_output(&mut command);
        assert!(output.status.success(), "{args:?}: {output:?}");
        String::from_utf8(output.stdout).unwrap()
    };
    fs::write(root.join("sto-3g.json"), include_bytes!("data/sto-3g.json")).unwrap();
    fs::write(
        root.join("6-31g.json"),
        include_bytes!("data/reference/RustiQ/basis_sets/6-31g.json"),
    )
    .unwrap();
    run(&["basis", "import", "sto-3g.json", "6-31g.json"]);
    fs::write(
        root.join("h2.xyz"),
        include_str!("../samples/h2/molecule.xyz"),
    )
    .unwrap();
    run(&["init", "h2.xyz", "-o", "calculation.toml"]);
    let hf = run(&[
        "run",
        "calculation.toml",
        "--no-auto-download",
        "--artifact",
        "h2.rustiq",
    ]);
    assert!(hf.contains("-1.116759"), "{hf}");
    let verified = run(&["artifact", "verify", "h2.rustiq"]);
    assert!(verified.contains("Verified 1 payload(s)"), "{verified}");
    run(&["init", "h2.xyz", "--mp2", "--force"]);
    let mp2 = run(&[
        "run",
        "calculation.toml",
        "--no-auto-download",
        "--format",
        "json",
    ]);
    let mp2: serde_json::Value = serde_json::from_str(&mp2).unwrap();
    let correlation = mp2["calculation"]["mp2"]["correlation_energy"]
        .as_f64()
        .unwrap();
    assert!((correlation - -0.013_138).abs() < 1e-6);
    fs::create_dir(root.join("inputs")).unwrap();
    fs::write(root.join("oh.xyz"), include_str!("../samples/oh/oh.xyz")).unwrap();
    fs::write(
        root.join("inputs/oh.toml"),
        include_str!("../samples/oh/sto-3g/calculation.toml"),
    )
    .unwrap();
    let oh = run(&["run", "inputs/oh.toml", "--no-auto-download"]);
    assert!(oh.contains("-74.362669"), "{oh}");
    fs::write(
        root.join("molecule.xyz"),
        include_str!("../samples/h2/molecule.xyz"),
    )
    .unwrap();
    fs::write(
        root.join("inputs/h2.toml"),
        include_str!("../samples/h2/sto-3g/calculation.toml"),
    )
    .unwrap();
    let copied = run(&["run", "inputs/h2.toml", "--no-auto-download"]);
    assert!(copied.contains("-1.116759"), "{copied}");
    fs::write(
        root.join("study.ncl"),
        include_str!("../samples/h2/study.ncl"),
    )
    .unwrap();
    let study = run(&["run", "study.ncl", "--no-auto-download"]);
    assert!(
        study.contains("2 succeeded, 0 non-converged, 0 failed"),
        "{study}"
    );
    assert!(study.contains("-1.116759"), "{study}");
    fs::remove_dir_all(root.join("data")).unwrap();
    fs::remove_file(root.join("h2.xyz")).unwrap();
    fs::remove_file(root.join("calculation.toml")).unwrap();
    let rerun = run(&["run", "h2.rustiq"]);
    assert!(rerun.contains("-1.116759"), "{rerun}");
}
