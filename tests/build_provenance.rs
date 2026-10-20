#![allow(
    clippy::unwrap_used,
    reason = "Tests fail on unexpected Git and filesystem errors"
)]

#[path = "../build/provenance.rs"]
mod provenance;

use std::{fs, path::Path, process::Command};

fn git(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(path)
        .args([
            "-c",
            "user.name=Build test",
            "-c",
            "user.email=build@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn instructions(path: &Path) -> String {
    // Cargo passes rustc-env values to test processes; vergen interprets those
    // as overrides. Discover each fixture repository without that inherited data.
    temp_env::with_vars_unset(
        [
            "VERGEN_GIT_SHA",
            "VERGEN_GIT_DIRTY",
            "VERGEN_IDEMPOTENT",
            "VERGEN_DEFAULT_ON_ERROR",
        ],
        || {
            let mut output = Vec::new();
            provenance::git_instructions(path)
                .unwrap()
                .emit_to(&mut output)
                .unwrap();
            String::from_utf8(output).unwrap()
        },
    )
}

#[test]
fn git_provenance_tracks_edits_reversions_commits_and_worktrees() {
    let root = tempfile::tempdir().unwrap();
    let repo = root.path().join("repo");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init"]);
    fs::write(repo.join("tracked"), "original\n").unwrap();
    git(&repo, &["add", "tracked"]);
    git(&repo, &["commit", "-m", "Initial commit"]);
    let first = git(&repo, &["rev-parse", "HEAD"]);
    let output = instructions(&repo);
    assert!(output.contains(&format!("VERGEN_GIT_SHA={first}\n")));
    assert!(output.contains("VERGEN_GIT_DIRTY=false\n"));

    fs::write(repo.join("untracked"), "ignored\n").unwrap();
    assert!(instructions(&repo).contains("VERGEN_GIT_DIRTY=false\n"));
    fs::write(repo.join("tracked"), "modified\n").unwrap();
    assert!(instructions(&repo).contains("VERGEN_GIT_DIRTY=true\n"));
    git(&repo, &["add", "tracked"]);
    assert!(instructions(&repo).contains("VERGEN_GIT_DIRTY=true\n"));
    git(&repo, &["reset", "--hard", "HEAD"]);
    assert!(instructions(&repo).contains("VERGEN_GIT_DIRTY=false\n"));

    fs::write(repo.join("tracked"), "new commit\n").unwrap();
    git(&repo, &["commit", "-am", "Update tracked file"]);
    let second = git(&repo, &["rev-parse", "HEAD"]);
    assert_ne!(first, second);
    assert!(instructions(&repo).contains(&format!("VERGEN_GIT_SHA={second}\n")));

    let worktree = root.path().join("worktree");
    git(
        &repo,
        &[
            "worktree",
            "add",
            "--detach",
            worktree.to_str().unwrap(),
            "HEAD",
        ],
    );
    assert!(instructions(&worktree).contains(&format!("VERGEN_GIT_SHA={second}\n")));
    fs::write(worktree.join("tracked"), "worktree edit\n").unwrap();
    assert!(instructions(&worktree).contains("VERGEN_GIT_DIRTY=true\n"));
}

#[test]
fn missing_repository_uses_unknown_placeholders() {
    let directory = tempfile::tempdir().unwrap();
    let output = instructions(directory.path());
    assert!(output.contains("VERGEN_GIT_SHA=VERGEN_IDEMPOTENT_OUTPUT\n"));
    assert!(output.contains("VERGEN_GIT_DIRTY=VERGEN_IDEMPOTENT_OUTPUT\n"));
}

#[test]
fn injected_provenance_must_be_a_full_hash_and_known_state() {
    for value in ["", "unknown", &"a".repeat(40), &"b".repeat(64)] {
        assert!(provenance::validate_override("RUSTIQ_BUILD_GIT_COMMIT", value).is_ok());
    }
    for value in ["abc123", "main", "bad\nvalue"] {
        assert!(provenance::validate_override("RUSTIQ_BUILD_GIT_COMMIT", value).is_err());
    }
    for value in ["", "clean", "dirty", "unknown"] {
        assert!(provenance::validate_override("RUSTIQ_BUILD_GIT_STATE", value).is_ok());
    }
    assert!(provenance::validate_override("RUSTIQ_BUILD_GIT_STATE", "false").is_err());
    assert!(provenance::validate_vergen_git_override("VERGEN_GIT_SHA", &"a".repeat(40)).is_ok());
    assert!(provenance::validate_vergen_git_override("VERGEN_GIT_SHA", "bad").is_err());
}
