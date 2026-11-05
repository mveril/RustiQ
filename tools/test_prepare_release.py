"""Exercise release preparation in disposable Git repositories."""

import runpy
from pathlib import Path

import pytest

MODULE = runpy.run_path(str(Path(__file__).with_name("prepare-release.py")))
prepare = MODULE["prepare"]
git = MODULE["git"]


@pytest.fixture
def repository(tmp_path):
    git(tmp_path, "init", "-b", "chore/prepare-version")
    git(tmp_path, "config", "user.email", "test@example.com")
    git(tmp_path, "config", "user.name", "Release test")
    (tmp_path / "crates/rustiq-core").mkdir(parents=True)
    (tmp_path / "Cargo.toml").write_text(
        '[package]\nversion = "1.2.3"\n[dependencies]\nrustiq-core = {version = "1.2.3"}\n'
    )
    (tmp_path / "crates/rustiq-core/Cargo.toml").write_text(
        '[package]\nversion = "1.2.3"\n'
    )
    (tmp_path / "flake.nix").write_text(MODULE["DEVELOPMENT_FLAG"] + "\n")
    git(tmp_path, "add", ".")
    git(tmp_path, "commit", "-m", "Initial state")
    return tmp_path


def test_release_tag_and_development_commit(repository):
    initial = git(repository, "rev-parse", "HEAD")
    prepare(repository, "v1.2.3")
    assert MODULE["RELEASE_FLAG"].strip() == git(repository, "show", "v1.2.3:flake.nix")
    assert MODULE["DEVELOPMENT_FLAG"] in (repository / "flake.nix").read_text()
    assert git(repository, "rev-parse", "HEAD^") == git(
        repository, "rev-parse", "v1.2.3"
    )
    assert git(repository, "rev-parse", "HEAD^^") == initial
    assert git(repository, "status", "--porcelain") == ""


@pytest.mark.parametrize("tag", ["v9.9.9", "invalid", "v01.2.3"])
def test_invalid_release_leaves_repository_unchanged(repository, tag):
    initial = git(repository, "rev-parse", "HEAD")
    with pytest.raises(ValueError):
        prepare(repository, tag)
    assert git(repository, "rev-parse", "HEAD") == initial
    assert git(repository, "status", "--porcelain") == ""


def test_dirty_tree_is_preserved(repository):
    (repository / "local.txt").write_text("local work")
    with pytest.raises(ValueError, match="local changes"):
        prepare(repository, "v1.2.3")
    assert (repository / "local.txt").read_text() == "local work"
    assert git(repository, "tag", "--list") == ""


def test_existing_tag_is_preserved(repository):
    git(repository, "tag", "v1.2.3")
    initial = git(repository, "rev-parse", "HEAD")
    with pytest.raises(ValueError, match="already exists"):
        prepare(repository, "v1.2.3")
    assert git(repository, "rev-parse", "HEAD") == initial


def test_task_branch_release(repository, capsys):
    git(repository, "switch", "-c", "chore/prepare-release")
    prepare(repository, "v1.2.3")
    assert git(repository, "branch", "--show-current") == "chore/prepare-release"
    assert git(repository, "rev-parse", "HEAD^") == git(repository, "rev-parse", "v1.2.3")
    output = capsys.readouterr().out
    assert "git push -u origin chore/prepare-release" in output
    assert "gh pr create --base main --head chore/prepare-release" in output
    assert "no squash or rebase" in output
    assert "git push --atomic origin main" not in output


@pytest.mark.parametrize("branch", ["main", "legacy", "chore/Invalid", None])
def test_invalid_branch_leaves_repository_unchanged(repository, branch):
    if branch is None:
        git(repository, "checkout", "--detach")
    else:
        git(repository, "switch", "-c", branch)
    initial = git(repository, "rev-parse", "HEAD")
    with pytest.raises(ValueError, match="branch"):
        prepare(repository, "v1.2.3")
    assert git(repository, "rev-parse", "HEAD") == initial
    assert git(repository, "tag", "--list") == ""
    assert git(repository, "status", "--porcelain") == ""
