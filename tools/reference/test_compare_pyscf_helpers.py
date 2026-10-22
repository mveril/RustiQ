"""Tests for reference helpers that do not import or execute PySCF."""

from __future__ import annotations

import json
import tomllib
from dataclasses import replace
from pathlib import Path
from typing import Any

import compare_pyscf
import pytest


@pytest.mark.parametrize("name", ["oh-sto-3g-uhf", "oh-sto-3g-uhf-mp2"])
def test_oh_sample_scf_threshold_matches_pyscf_reference(name: str) -> None:
    """Guard the explicitly aligned numerical thresholds of OH inputs."""
    case = next(case for case in compare_pyscf.CASES if case.name == name)
    config = tomllib.loads(case.runfile.read_text(encoding="utf-8"))
    # Identical numbers do not imply identical SCF stopping criteria.
    assert config["method"]["hf"]["convergence_threshold"] == case.conv_tol


@pytest.mark.parametrize(
    ("name", "counts"),
    [
        ("lih-6-31g-rhf", (2, 2)),
        ("co-sto-3g-rhf-mp2", (7, 7)),
        ("h2o-cc-pvdz-rhf-mp2", (5, 5)),
        ("oh-6-31g-uhf-mp2", (5, 4)),
    ],
)
def test_new_case_scientific_metadata(name: str, counts: tuple[int, int]) -> None:
    case = next(case for case in compare_pyscf.CASES if case.name == name)
    config = tomllib.loads(case.runfile.read_text(encoding="utf-8"))
    assert compare_pyscf.electron_counts(case) == counts
    assert case.bse_reference
    assert case.frozen_orbitals == 0
    assert min(counts) > 0 and max(counts) < case.ao_dimension
    assert config["method"]["hf"]["convergence_threshold"] == case.conv_tol
    assert config["method"]["hf"]["max_iterations"] == case.max_cycle
    if case.mp2:
        assert config["method"]["mp2"]["frozen_orbitals"] == 0


def test_rustiq_result_rejects_malformed_json(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(
        compare_pyscf, "run_command", lambda *args, **kwargs: "not JSON"
    )
    with pytest.raises(RuntimeError, match="Could not parse RustiQ JSON output"):
        compare_pyscf.rustiq_result(compare_pyscf.CASES[0], {})


@pytest.mark.parametrize(
    "payload",
    [
        {"calculation": {"hf": {}}},
        {"schema_version": 2, "calculation": {"hf": {}}},
        {"schema_version": 1},
        {"schema_version": 1, "calculation": []},
    ],
)
def test_rustiq_result_rejects_incompatible_json_contract(
    monkeypatch: pytest.MonkeyPatch, payload: dict[str, Any]
) -> None:
    monkeypatch.setattr(
        compare_pyscf, "run_command", lambda *args, **kwargs: json.dumps(payload)
    )
    with pytest.raises(
        (TypeError, ValueError), match="RustiQ JSON output|schema_version"
    ):
        compare_pyscf.rustiq_result(compare_pyscf.CASES[0], {})


def test_frozen_core_runfile_escapes_windows_paths(tmp_path: Path) -> None:
    original = next(case for case in compare_pyscf.CASES if case.frozen_core_reference)
    windows_path = Path(r"C:\Users\chemist name\work\RustiQ\samples\oh\oh.xyz")
    case = replace(original, xyz=windows_path)
    generated = compare_pyscf.frozen_core_case(case, "32 KiB", tmp_path)
    parsed = tomllib.loads(generated.runfile.read_text(encoding="utf-8"))
    assert parsed["molecule"]["geometry"] == str(windows_path)


def test_resolved_configuration_validation() -> None:
    case = next(case for case in compare_pyscf.CASES if case.name == "lih-6-31g-rhf")
    coordinates = [
        line.split()
        for line in compare_pyscf.load_xyz_body(case.xyz).splitlines()
        if line.strip()
    ]
    geometry = "2\nresolved\n" + "\n".join(
        f"{row[0]} " + " ".join(str(float(value) / 0.529177210903) for value in row[1:])
        for row in coordinates
    )
    configuration = f"""[molecule]
charge = {case.charge}
multiplicity = {case.spin + 1}
units = "Bohr"

[basis]
name = "{case.basis}"

[method.hf]
method = "{case.method}"
convergence_threshold = {case.conv_tol}
max_iterations = {case.max_cycle}
"""
    resolved = {"configuration": configuration, "geometry": geometry}
    compare_pyscf.validate_resolved(case, resolved)
    bad_lines = geometry.splitlines()
    first_atom = bad_lines[2].split()
    first_atom[1] = str(float(first_atom[1]) + 0.1)
    bad_lines[2] = " ".join(first_atom)
    bad = dict(resolved, geometry="\n".join(bad_lines))
    with pytest.raises(AssertionError):
        compare_pyscf.validate_resolved(case, bad)
