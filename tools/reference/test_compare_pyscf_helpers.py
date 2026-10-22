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
    """Guard the explicitly aligned numerical thresholds of the OH reference inputs."""
    case = next(case for case in compare_pyscf.CASES if case.name == name)
    config = tomllib.loads(case.runfile.read_text(encoding="utf-8"))

    # Identical numbers do not imply identical SCF stopping criteria.
    assert config["method"]["hf"]["convergence_threshold"] == case.conv_tol


def test_rustiq_result_rejects_malformed_json(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def malformed_output(*args: Any, **kwargs: Any) -> str:
        return "not JSON"

    monkeypatch.setattr(compare_pyscf, "run_command", malformed_output)

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

    with pytest.raises((TypeError, ValueError), match="RustiQ JSON output|schema_version"):
        compare_pyscf.rustiq_result(compare_pyscf.CASES[0], {})


def test_frozen_core_runfile_preserves_windows_geometry_path(tmp_path: Path) -> None:
    case = next(case for case in compare_pyscf.CASES if case.frozen_core_reference)
    case = replace(case, xyz=Path(r"C:\Users\chemist name\water.xyz"))
    generated = compare_pyscf.frozen_core_case(case, "32 KiB", tmp_path)
    config = tomllib.loads(generated.runfile.read_text(encoding="utf-8"))
    assert config["molecule"]["geometry"] == str(case.xyz)
