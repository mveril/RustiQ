import importlib.util
import json
import sys
from pathlib import Path

import pytest

spec = importlib.util.spec_from_file_location(
    "publish_docs", Path(__file__).with_name("publish-docs.py")
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def test_publication_lifecycle(tmp_path):
    book, site = tmp_path / "book", tmp_path / "site"
    book.mkdir()
    (book / "index.html").write_text("first")
    (book / "404.html").write_text('<base href="./">')
    (book / "404.html").chmod(0o444)
    site.mkdir()
    (site / "legacy.html").write_text("legacy")
    commit = "a" * 40
    module.publish(book, site, commit)
    assert (site / "dev/index.html").read_text() == "first"
    assert not (site / "stable").exists()
    module.publish(book, site, commit, "v0.1.0-beta.1")
    snapshot = (site / "v0.1.0-beta.1/index.html").read_bytes()
    (book / "index.html").write_text("second")
    module.publish(book, site, "b" * 40, "v0.1.0-beta.2")
    module.publish(book, site, "c" * 40, "v0.1.0")
    assert (site / "v0.1.0-beta.1/index.html").read_bytes() == snapshot
    assert "../v0.1.0-beta.2/" in (site / "beta/index.html").read_text()
    assert list((site / "beta").iterdir()) == [site / "beta/index.html"]
    assert "../v0.1.0/" in (site / "stable/index.html").read_text()
    assert "dev/legacy.html" in (site / "legacy.html").read_text()
    (book / "index.html").write_text("first")
    module.publish(book, site, commit, "v0.1.0-beta.1")
    module.publish(book, site, commit, "v0.1.0-beta.1")
    data = json.loads((site / "versions.json").read_text())
    assert [v["version"] for v in data["versions"]] == [
        "v0.1.0",
        "v0.1.0-beta.2",
        "v0.1.0-beta.1",
    ]
    assert data["latest_beta"] == "v0.1.0-beta.2"
    assert data["latest_stable"] == "v0.1.0"
    assert data["versions"][-1]["commit"] == commit
    with pytest.raises(ValueError, match="conflict"):
        module.publish(book, site, "d" * 40, "v0.1.0-beta.1")
    (book / "index.html").write_text("changed")
    with pytest.raises(ValueError, match="conflict"):
        module.publish(book, site, commit, "v0.1.0-beta.1")


@pytest.mark.parametrize(
    "version", ["v01.0.0", "v1.0.0-beta.01", "../dev", "v1.0", "v1.0.0+build"]
)
def test_invalid_version(version):
    with pytest.raises(ValueError):
        module.version_key(version)


def test_semver_ordering():
    versions = [
        "v1.0.0",
        "v1.0.0-rc.1",
        "v1.0.0-beta.11",
        "v1.0.0-beta.2",
        "v1.0.0-beta",
        "v0.9.0",
    ]
    assert sorted(versions, key=module.version_key, reverse=True) == versions


def test_legacy_links_and_conflict_leave_site_unchanged(tmp_path):
    book, site = tmp_path / "book", tmp_path / "site"
    book.mkdir()
    (book / "index.html").write_text("book")
    (book / "user-guide").mkdir()
    (book / "user-guide/hartree-fock.html").write_text("new")
    (site / "getting-started").mkdir(parents=True)
    (site / "getting-started/installation.html").write_text("old")
    (site / "obsolete.js").write_text("old asset")
    module.publish(book, site, "a" * 40, "v1.0.0-beta.1")
    assert (
        "../dev/getting-started/installation.html"
        in (site / "getting-started/installation.html").read_text()
    )
    assert (
        "../dev/user-guide/hartree-fock.html"
        in (site / "user-guide/hartree-fock.html").read_text()
    )
    assert not (site / "obsolete.js").exists()
    before = {
        p.relative_to(site): p.read_bytes() for p in site.rglob("*") if p.is_file()
    }
    with pytest.raises(ValueError, match="conflict"):
        module.publish(book, site, "b" * 40, "v1.0.0-beta.1")
    assert before == {
        p.relative_to(site): p.read_bytes() for p in site.rglob("*") if p.is_file()
    }


def site_tree(site):
    return {p.relative_to(site): p.read_bytes() for p in site.rglob("*") if p.is_file()}


def redirect_url(page, requested):
    import re
    from html import unescape
    from urllib.parse import urljoin

    target = re.search(r'content="0; url=([^"]+)"', page.read_text()).group(1)
    return urljoin(requested, unescape(target))


@pytest.mark.parametrize("release_first", [False, True])
def test_pages_404_resolution(tmp_path, release_first):
    book, site = tmp_path / "book", tmp_path / "site"
    book.mkdir()
    (book / "index.html").write_text("manual")
    (book / "404.html").write_text('<base href="./">missing')
    original = site_tree(book)
    module.publish(book, site, "a" * 40, "v1.0.0" if release_first else None)
    fallback = "v1.0.0" if release_first else "dev"
    for missing in [
        "missing.html",
        "dev/missing.html",
        "v1.0.0/missing.html",
        "dev/a/b/c/missing.html",
        "v1.0.0/a/b/c/missing.html",
        "a/b/c/d/missing.html",
        "404.html",
        "dev/404.html",
    ]:
        requested = "https://mveril.github.io/RustiQ/" + missing
        resolved = redirect_url(site / "404.html", requested)
        assert resolved == f"https://mveril.github.io/RustiQ/{fallback}/"
        assert resolved != requested
        assert (site / fallback / "index.html").read_text() == "manual"
    assert site_tree(book) == original
    assert (
        '<base href="/RustiQ/' + fallback + '/">'
        in (site / fallback / "404.html").read_text()
    )
    # Upgrade an already migrated site's old, broken root 404 too.
    (site / "404.html").write_text("old relative redirect")
    module.publish(book, site, "a" * 40, "v1.0.0" if release_first else None)
    assert redirect_url(site / "404.html", requested) == resolved


@pytest.mark.parametrize("seeded", [False, True])
def test_initial_migration_provenance_and_idempotence(tmp_path, capsys, seeded):
    book, site = tmp_path / "book", tmp_path / "site"
    (book / "guide").mkdir(parents=True)
    (book / "index.html").write_text("manual")
    (book / "guide/current.html").write_text("current")
    if seeded:
        (site / "old").mkdir(parents=True)
        (site / "old/removed.html").write_text("historical")
        (site / "index.html").write_text("historical index")
    module.publish(book, site, "a" * 40)
    warning = capsys.readouterr().err
    assert ("no historical root HTML" in warning) == (not seeded)
    data = json.loads((site / "versions.json").read_text())
    assert data["legacy_source"] == ("existing-root" if seeded else "current-book-only")
    assert (site / "old/removed.html").exists() == seeded
    paths = ["guide/current.html"] + (["old/removed.html"] if seeded else [])
    for path in paths:
        assert redirect_url(site / path, "https://mveril.github.io/RustiQ/" + path) == (
            "https://mveril.github.io/RustiQ/dev/" + path
        )
    before = site_tree(site)
    module.publish(book, site, "a" * 40)
    assert site_tree(site) == before
    assert not capsys.readouterr().err


@pytest.mark.skipif(
    sys.platform != "linux",
    reason="GitHub Actions documentation publishing runs on Linux",
)
@pytest.mark.parametrize(
    ("tag", "on_main", "error"),
    [
        ("v1.0.0", True, None),
        ("v1.0.0-beta.1", True, None),
        ("v1.0.0", False, "release tags must point to a commit on main"),
        ("v1.0.1", True, "does not match package version"),
        ("v01.0.0", True, "invalid release version"),
        ("v1.0.0-beta.01", True, "invalid release version"),
    ],
)
def test_workflow_release_validation(tmp_path, tag, on_main, error):
    import os
    import shutil
    import subprocess
    import textwrap

    root = Path(__file__).resolve().parents[1]
    workflow = (root / ".github/workflows/docs-publish.yml").read_text()
    validation = workflow.split("      - name: Validate release tag\n", 1)[1]
    script = textwrap.dedent(
        validation.split("        run: |\n", 1)[1].split("      - name:", 1)[0]
    )

    def git(*args):
        return subprocess.run(
            ["git", *args], cwd=tmp_path, check=True, capture_output=True, text=True
        )

    git("init", "-b", "main")
    git("config", "user.name", "Test")
    git("config", "user.email", "test@example.invalid")
    (tmp_path / "Cargo.toml").write_text('[package]\nversion = "1.0.0"\n')
    (tmp_path / "tools").mkdir()
    shutil.copy(root / "tools/publish-docs.py", tmp_path / "tools")
    shutil.copy(root / "tools/check-release.py", tmp_path / "tools")
    (tmp_path / "crates/rustiq-core").mkdir(parents=True)
    release_version = "1.0.0-beta.1" if tag == "v1.0.0-beta.1" else "1.0.0"
    (tmp_path / "Cargo.toml").write_text(
        f'[package]\nversion = "{release_version}"\n[dependencies]\nrustiq-core = {{version = "{release_version}"}}\n'
    )
    (tmp_path / "crates/rustiq-core/Cargo.toml").write_text(
        f'[package]\nversion = "{release_version}"\n'
    )
    (tmp_path / "CITATION.cff").write_text(f"version: {release_version}\n")

    git("add", ".")
    git("commit", "-m", "Initial main")
    # Use the repository itself as a local remote, exercising the actual fetch.
    git("remote", "add", "origin", str(tmp_path))
    if not on_main:
        git("checkout", "-b", "side")
        git("commit", "--allow-empty", "-m", "Outside main")
    if tag == "v1.0.0-beta.1":
        (tmp_path / "Cargo.toml").write_text(
            '[package]\nversion = "1.0.0-beta.1"\n[dependencies]\nrustiq-core = {version = "1.0.0-beta.1"}\n'
        )
        git("add", "Cargo.toml")
        git("commit", "--allow-empty", "-m", "Prerelease")
    result = subprocess.run(
        ["bash", "-c", script],
        check=False,
        cwd=tmp_path,
        env={**os.environ, "RELEASE_TAG": tag},
        capture_output=True,
        text=True,
    )
    if error:
        assert result.returncode != 0
        assert error in result.stderr
    else:
        assert result.returncode == 0, result.stderr


def test_aliases_follow_semver_not_publication_order(tmp_path):
    book, site = tmp_path / "book", tmp_path / "site"
    book.mkdir()
    (book / "index.html").write_text("manual")
    versions = ["v1.0.0-beta.11", "v1.0.0", "v1.0.0-beta.2", "v0.9.0"]
    for version in versions:
        module.publish(book, site, "a" * 40, version)
    data = json.loads((site / "versions.json").read_text())
    assert [entry["version"] for entry in data["versions"]] == sorted(
        versions, key=module.version_key, reverse=True
    )
    assert redirect_url(
        site / "beta/index.html", "https://mveril.github.io/RustiQ/beta/"
    ) == ("https://mveril.github.io/RustiQ/v1.0.0-beta.11/")
    assert redirect_url(
        site / "stable/index.html", "https://mveril.github.io/RustiQ/stable/"
    ) == ("https://mveril.github.io/RustiQ/v1.0.0/")

    before = site_tree(site)
    module.publish(book, site, "a" * 40, "v0.9.0")
    assert site_tree(site) == before
