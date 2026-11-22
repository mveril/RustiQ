#![allow(
    clippy::unwrap_used,
    reason = "Integration tests fail immediately on unexpected fixture errors"
)]

use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn geometry_command(root: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rustiq"))
        .current_dir(root)
        .args(["--color", "never", "geometry", "translate", "input.xyz"])
        .args(extra)
        .output()
        .unwrap()
}

fn fixture() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    fs::write(
        directory.path().join("input.xyz"),
        "2\nH2\nH 0 0 0\nH 0 0 0.74\n",
    )
    .unwrap();
    directory
}

#[test]
fn geometry_outputs_require_force_and_preserve_existing_data() {
    let directory = fixture();
    let root = directory.path();
    let original = b"previous geometry";
    fs::write(root.join("output.xyz"), original).unwrap();

    let rejected = geometry_command(root, &["--dx", "1", "-o", "output.xyz"]);
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("--force"));
    assert_eq!(fs::read(root.join("output.xyz")).unwrap(), original);

    let accepted = geometry_command(root, &["--dx", "1", "-o", "output.xyz", "--force"]);
    assert!(
        accepted.status.success(),
        "{}",
        String::from_utf8_lossy(&accepted.stderr)
    );
    let written = fs::read(root.join("output.xyz")).unwrap();
    assert_ne!(written, original);
    assert!(String::from_utf8(written).unwrap().contains("H"));

    let refused_again = geometry_command(root, &["--dx", "2", "-o", "output.xyz"]);
    assert!(!refused_again.status.success());
}

#[test]
fn failed_geometry_transform_does_not_change_destination() {
    let directory = fixture();
    let root = directory.path();
    fs::write(root.join("output.xyz"), b"untouched").unwrap();
    fs::write(root.join("input.xyz"), b"invalid geometry").unwrap();
    let failed = geometry_command(root, &["--dx", "1", "-o", "output.xyz", "--force"]);
    assert!(!failed.status.success());
    assert_eq!(fs::read(root.join("output.xyz")).unwrap(), b"untouched");
}

#[cfg(unix)]
#[test]
fn forced_geometry_output_replaces_target_without_removing_alias() {
    let directory = fixture();
    let root = directory.path();
    fs::write(root.join("target.xyz"), b"old target").unwrap();
    std::os::unix::fs::symlink("target.xyz", root.join("alias.xyz")).unwrap();

    let refused = geometry_command(root, &["-o", "alias.xyz"]);
    assert!(!refused.status.success());
    assert_eq!(fs::read(root.join("target.xyz")).unwrap(), b"old target");

    let written = geometry_command(root, &["--dx", "1", "-o", "alias.xyz", "--force"]);
    assert!(
        written.status.success(),
        "{}",
        String::from_utf8_lossy(&written.stderr)
    );
    assert!(fs::symlink_metadata(root.join("alias.xyz"))
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        fs::read(root.join("alias.xyz")).unwrap(),
        fs::read(root.join("target.xyz")).unwrap()
    );
    assert_ne!(fs::read(root.join("target.xyz")).unwrap(), b"old target");
}

#[cfg(unix)]
#[test]
fn dangling_geometry_alias_is_never_replaced() {
    let directory = fixture();
    let root = directory.path();
    std::os::unix::fs::symlink("missing.xyz", root.join("dangling.xyz")).unwrap();
    let output = geometry_command(root, &["-o", "dangling.xyz", "--force"]);
    assert!(!output.status.success());
    assert!(fs::symlink_metadata(root.join("dangling.xyz"))
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(!root.join("missing.xyz").exists());
}
