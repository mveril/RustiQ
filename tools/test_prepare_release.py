"""Release candidate checks must not modify a checkout."""

import runpy
from pathlib import Path

import pytest

candidate = runpy.run_path(str(Path(__file__).with_name("check-release.py")))[
    "candidate"
]


def manifests(root, version, core=None, dependency=None):
    (root / "crates/rustiq-core").mkdir(parents=True)
    (root / "CITATION.cff").write_text(f"version: {version}\n")
    (root / "Cargo.toml").write_text(
        f'[package]\nversion = "{version}"\n[dependencies]\nrustiq-core = {{version = "{dependency or version}"}}\n'
    )
    (root / "crates/rustiq-core/Cargo.toml").write_text(
        f'[package]\nversion = "{core or version}"\n'
    )


@pytest.mark.parametrize(
    "version", ["1.2.3-dev", "1.2.3-dev.1", "1.2.3-alpha.1.dev", "1.2.3-beta.01"]
)
def test_rejects_development_and_invalid_versions(tmp_path, version):
    manifests(tmp_path, version)
    with pytest.raises(ValueError):
        candidate(tmp_path)


@pytest.mark.parametrize("version", ["1.2.3", "1.2.3-alpha.1", "1.2.3-beta.1"])
def test_accepts_release_without_mutation(tmp_path, version):
    manifests(tmp_path, version)
    before = {p: p.read_bytes() for p in tmp_path.rglob("Cargo.toml")}
    assert candidate(tmp_path) == version
    assert before == {p: p.read_bytes() for p in before}


def test_rejects_mismatched_workspace(tmp_path):
    manifests(tmp_path, "1.2.3", core="1.2.4")
    with pytest.raises(ValueError, match="versions differ"):
        candidate(tmp_path)


def test_rejects_mismatched_dependency(tmp_path):
    manifests(tmp_path, "1.2.3", dependency="1.2.2")
    with pytest.raises(ValueError, match="dependency"):
        candidate(tmp_path)


@pytest.mark.parametrize(
    "existing",
    [
        "git/ref/tags",
        "releases/tags",
        "rustiq-chem-core",
        "rustiq-chem",
        "versions.json",
    ],
)
def test_rejects_each_existing_publication(monkeypatch, existing):
    available = runpy.run_path(str(Path(__file__).with_name("check-release.py")))[
        "available"
    ]

    def lookup(url, token=None):
        if existing not in url:
            return None
        return (
            {"versions": [{"version": "v1.2.3"}]} if existing == "versions.json" else {}
        )

    monkeypatch.setitem(available.__globals__, "lookup", lookup)
    with pytest.raises(ValueError, match="already"):
        available("1.2.3", "mveril/RustiQ")


def test_lookup_failure_does_not_allow_publication(monkeypatch):
    available = runpy.run_path(str(Path(__file__).with_name("check-release.py")))[
        "available"
    ]

    def lookup(url, token=None):
        raise OSError("service unavailable")

    monkeypatch.setitem(available.__globals__, "lookup", lookup)
    with pytest.raises(OSError, match="unavailable"):
        available("1.2.3", "mveril/RustiQ")


def test_rejects_mismatched_citation(tmp_path):
    manifests(tmp_path, "1.2.3")
    (tmp_path / "CITATION.cff").write_text("version: 1.2.2\n")
    with pytest.raises(ValueError, match="citation version"):
        candidate(tmp_path)
