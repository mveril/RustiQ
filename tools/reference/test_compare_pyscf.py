from __future__ import annotations

import csv
import io
import json
import sys
from pathlib import Path

import pytest

import compare_pyscf


@pytest.mark.parametrize(
    ("payload", "expected_message"),
    [
        (
            {"schema_version": 2, "calculation": {"hf": {"converged": True}}},
            "Unsupported RustiQ JSON schema",
        ),
        (
            {"schema_version": 1, "calculation": {"hf": {"converged": False}}},
            "RustiQ did not converge",
        ),
        (
            {"schema_version": 1, "calculation": {"hf": {}}},
            "Could not parse RustiQ JSON output",
        ),
    ],
    ids=["unsupported-schema", "not-converged", "missing-convergence"],
)
def test_rustiq_result_rejects_invalid_results(
    monkeypatch: pytest.MonkeyPatch,
    payload: dict[str, object],
    expected_message: str,
) -> None:
    monkeypatch.setattr(
        compare_pyscf,
        "run_command",
        lambda *args, **kwargs: json.dumps(payload),
    )

    with pytest.raises(RuntimeError, match=expected_message):
        compare_pyscf.rustiq_result(compare_pyscf.CASES[0], {})


def test_rustiq_result_accepts_converged_result(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    payload = {
        "schema_version": 1,
        "calculation": {"hf": {"converged": True, "total_energy": -1.0}},
    }
    monkeypatch.setattr(
        compare_pyscf,
        "run_command",
        lambda *args, **kwargs: json.dumps(payload),
    )

    assert compare_pyscf.rustiq_result(compare_pyscf.CASES[0], {}) == payload


@pytest.mark.parametrize(
    ("case_name", "expected_name"),
    [
        ("plain-case", "plain-case"),
        ("case,with,commas", "case,with,commas"),
    ],
    ids=["plain-field", "comma-separated-field"],
)
def test_main_writes_parseable_csv(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    case_name: str,
    expected_name: str,
) -> None:
    case = compare_pyscf.ReferenceCase(
        name=case_name,
        runfile=tmp_path / "calculation.toml",
        xyz=tmp_path / "molecule.xyz",
        basis="sto-3g",
        charge=0,
        spin=0,
        method="rhf",
        conv_tol=1e-10,
        max_cycle=80,
        tolerance=1e-9,
    )
    data_home = tmp_path / "data-home"
    data_home.mkdir()
    rustiq = {
        "schema_version": 1,
        "calculation": {"hf": {"converged": True, "total_energy": -1.0}},
    }
    monkeypatch.setattr(compare_pyscf, "CASES", [case])
    monkeypatch.setattr(
        compare_pyscf,
        "prepare_rustiq_env",
        lambda: {"RUSTIQ_DATA_HOME": str(data_home)},
    )
    monkeypatch.setattr(compare_pyscf, "rustiq_result", lambda *_: rustiq)
    monkeypatch.setattr(compare_pyscf, "pyscf_result", lambda *_: (-1.0, None))
    monkeypatch.setattr(sys, "argv", ["compare_pyscf.py"])

    assert compare_pyscf.main() == 0

    rows = list(csv.DictReader(io.StringIO(capsys.readouterr().out)))
    assert len(rows) == 1
    assert rows[0]["case"] == expected_name
    assert rows[0]["status"] == "ok"
