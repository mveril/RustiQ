from __future__ import annotations

import json
import math
from pathlib import Path

import compare_pyscf
import pytest
import report


def test_hf_mp2_and_spin_differences_and_markdown() -> None:
    case = next(c for c in compare_pyscf.CASES if c.name == "oh-sto-3g-uhf-mp2")
    rustiq = {
        "schema_version": 1,
        "calculation": {
            "hf": {
                "converged": True,
                "total_energy": -75.0,
                "method": "UHF",
                "orthogonalization": {"ao_basis_dimension": case.ao_dimension},
                "spin": {
                    "s_squared": 0.752,
                    "ideal_s_squared": 0.75,
                    "spin_contamination": 0.002,
                },
            },
            "mp2": {
                "method": "UHF-MP2",
                "correlation_energy": -0.2,
                "total_energy": -75.2,
            },
        },
    }
    result = report.validate_case(
        case, rustiq, (-75.0 + 1e-9, -0.2 + 1e-10, 0.752 + 1e-6), [], True
    )
    assert math.isclose(result["hf_absolute_difference_hartree"], 1e-9, abs_tol=1e-12)
    assert math.isclose(result["mp2_absolute_difference_hartree"], 1e-10, abs_tol=1e-12)
    assert math.isclose(result["s_squared_absolute_difference"], 1e-6, abs_tol=1e-12)
    assert result["status"] == "PASS"
    md = report.render_markdown({"cases": [result], "metadata": {}})
    assert "total molecular HF energies" in md
    assert "correlation energies only" in md


def test_missing_and_non_finite_values_fail() -> None:
    case = compare_pyscf.CASES[0]
    result = report.validate_case(
        case,
        {
            "schema_version": 1,
            "calculation": {
                "hf": {
                    "converged": True,
                    "total_energy": float("nan"),
                    "method": "RHF",
                    "orthogonalization": {"ao_basis_dimension": case.ao_dimension},
                }
            },
        },
        (float("inf"), None, None),
        [],
    )
    assert result["rustiq_hf_total_energy_hartree"] is None
    assert result["pyscf_hf_total_energy_hartree"] is None
    assert result["status"] == "FAIL"
    missing = report.validate_case(case, None, None, ["calculation failed"])
    assert missing["status"] == "FAIL"
    assert missing["errors"]


def test_failed_tolerance_is_reported() -> None:
    case = compare_pyscf.CASES[0]
    row = report.validate_case(
        case,
        {
            "schema_version": 1,
            "calculation": {
                "hf": {
                    "converged": True,
                    "total_energy": -1.0,
                    "method": "RHF",
                    "orthogonalization": {"ao_basis_dimension": case.ao_dimension},
                }
            },
        },
        (-0.9, None, None),
        [],
    )
    assert not row["hf_pass"]
    assert row["status"] == "FAIL"


def valid_rustiq(case: compare_pyscf.ReferenceCase, *, energy: float = -75.0) -> dict:
    hf = {
        "converged": True,
        "total_energy": energy,
        "method": case.method.upper(),
        "orthogonalization": {"ao_basis_dimension": case.ao_dimension},
    }
    calculation = {"hf": hf}
    if case.method == "uhf":
        hf["spin"] = {
            "s_squared": 0.75,
            "ideal_s_squared": 0.75,
            "spin_contamination": 0.0,
        }
    if case.mp2:
        calculation["mp2"] = {
            "method": f"{case.method.upper()}-MP2",
            "correlation_energy": -0.2,
            "total_energy": energy - 0.2,
        }
    return {"schema_version": 1, "calculation": calculation}


@pytest.mark.parametrize("convergence", [False, None])
def test_pyscf_nonconvergence_or_unknown_fails_even_when_energies_match(
    convergence,
) -> None:
    case = compare_pyscf.CASES[0]
    result = report.validate_case(
        case, valid_rustiq(case), (-75.0, None, None), [], convergence
    )
    assert result["hf_absolute_difference_hartree"] == 0.0
    assert not result["hf_pass"]
    assert result["status"] == "FAIL"
    assert result["pyscf_hf_converged"] is convergence


@pytest.mark.parametrize("version", [None, 2, True])
def test_missing_or_unsupported_schema_version_fails(version) -> None:
    case = compare_pyscf.CASES[0]
    rustiq = valid_rustiq(case)
    if version is None:
        rustiq.pop("schema_version")
    else:
        rustiq["schema_version"] = version
    errors: list[str] = []
    result = report.validate_case(case, rustiq, (-75.0, None, None), errors, True)
    assert result["status"] == "FAIL"
    assert any("schema_version" in error for error in errors)


@pytest.mark.parametrize(
    "payload", [{"schema_version": 1}, {"schema_version": 1, "calculation": []}]
)
def test_malformed_json_structure_or_missing_calculation_section_fails(payload) -> None:
    case = compare_pyscf.CASES[0]
    result = report.validate_case(case, payload, (-75.0, None, None), [], True)
    assert result["status"] == "FAIL"
    assert result["errors"]


@pytest.mark.parametrize(
    ("case_name", "rustiq_change", "pyscf", "converged", "diagnostic"),
    [
        (
            "h2-sto-3g-rhf",
            lambda data: data["calculation"]["hf"].update(total_energy=float("inf")),
            (-75.0, None, None),
            True,
            "HF energy",
        ),
        (
            "h2-sto-3g-rhf-mp2",
            lambda data: data["calculation"]["mp2"].update(
                correlation_energy=float("nan")
            ),
            (-75.0, -0.2, None),
            True,
            "MP2 correlation",
        ),
        (
            "oh-sto-3g-uhf",
            lambda data: data["calculation"]["hf"].update(
                spin={"s_squared": float("nan")}
            ),
            (-75.0, None, 0.75),
            True,
            "Spin expectation",
        ),
    ],
)
def test_non_finite_scientific_values_fail(
    case_name, rustiq_change, pyscf, converged, diagnostic
) -> None:
    case = next(item for item in compare_pyscf.CASES if item.name == case_name)
    rustiq = valid_rustiq(case)
    rustiq_change(rustiq)
    errors: list[str] = []
    result = report.validate_case(case, rustiq, pyscf, errors, converged)
    assert result["status"] == "FAIL"
    assert any(diagnostic in error for error in errors)


def test_recorded_execution_error_forces_failure() -> None:
    case = compare_pyscf.CASES[0]
    errors = ["RustiQ calculation failed: synthetic diagnostic"]
    result = report.validate_case(
        case, valid_rustiq(case), (-75.0, None, None), errors, True
    )
    assert result["hf_pass"]
    assert result["status"] == "FAIL"


def test_combined_uhf_mp2_spin_validation_and_failed_case_serialization() -> None:
    case = next(
        item for item in compare_pyscf.CASES if item.name == "oh-sto-3g-uhf-mp2"
    )
    result = report.validate_case(
        case, valid_rustiq(case), (-75.0, -0.2, 0.75), [], True
    )
    assert result["hf_pass"] and result["mp2_pass"] and result["spin_pass"]
    assert result["status"] == "PASS"
    failed = report.validate_case(
        case,
        valid_rustiq(case),
        (-75.0, -0.2, 0.75),
        ["recorded execution error"],
        True,
    )
    payload = json.loads(json.dumps({"cases": [failed]}, allow_nan=False))
    assert payload["cases"][0]["status"] == "FAIL"
    markdown = report.render_markdown({**payload, "metadata": {}})
    assert "recorded execution error" in markdown
    assert "0/1 cases passed" in markdown


def test_json_serialization_preserves_full_precision_and_rejects_nan(
    tmp_path: Path,
) -> None:
    payload = {"energy": -1.2345678901234567}
    path = tmp_path / "report.json"
    path.write_text(json.dumps(payload, allow_nan=False), encoding="utf-8")
    assert json.loads(path.read_text())["energy"] == payload["energy"]


def test_markdown_scientific_notation_and_failure_detail() -> None:
    item = {
        "case_id": "failed",
        "molecule": "H2",
        "geometry_path": "samples/h2/molecule.xyz",
        "basis": "sto-3g",
        "charge": 0,
        "spin_n_alpha_minus_n_beta": 0,
        "ao_basis_dimension": 2,
        "hf_method": "RHF",
        "rustiq_hf_total_energy_hartree": None,
        "pyscf_hf_total_energy_hartree": None,
        "hf_absolute_difference_hartree": None,
        "hf_tolerance_hartree": 1e-10,
        "hf_pass": False,
        "status": "FAIL",
        "errors": ["HF energy comparison failed"],
    }
    md = report.render_markdown({"cases": [item], "metadata": {}})
    assert "0/1 cases passed" in md
    assert "HF energy comparison failed" in md
    assert "—" in md


def test_molecular_formula_uses_geometry_atoms() -> None:
    case = next(c for c in compare_pyscf.CASES if c.name == "h2o-sto-3g-rhf")
    assert report.molecular_formula(case.xyz) == "H2O"
    h2_case = compare_pyscf.CASES[0]
    assert report.molecular_formula(h2_case.xyz) == "H2"


def test_integral_section_includes_failures_and_metrics():
    row = {
        "case_id": "h2-sto-3g",
        "representation": "spherical",
        "nao": 2,
        "family": "ERI",
        "count": 6,
        "max_absolute_error": 1e-5,
        "rms_absolute_error": 1e-6,
        "absolute_tolerance": 1e-10,
        "status": "FAIL",
    }
    rendered = report.render_markdown(
        {
            "cases": [],
            "metadata": {},
            "integrals": {
                "passed": False,
                "cases": [row],
                "cargo_error": "fixture mismatch",
            },
        }
    )
    assert "Independent AO integrals: FAIL" in rendered
    assert "fixture mismatch" in rendered
    assert "1.00000000000000008e-05" in rendered


@pytest.mark.parametrize("wrong_total", [None, float("nan"), -75.3])
def test_mp2_total_corruption_cannot_pass(wrong_total) -> None:
    case = next(c for c in compare_pyscf.CASES if c.name == "co-sto-3g-rhf-mp2")
    rustiq = valid_rustiq(case)
    rustiq["calculation"]["mp2"]["total_energy"] = wrong_total
    row = report.validate_case(case, rustiq, (-75.0, -0.2, None), [], True)
    assert row["mp2_pass"]  # Correlation agreement alone must not hide a bad total.
    assert not row["mp2_total_pass"]
    assert row["status"] == "FAIL"


def test_input_metadata_survives_missing_pyscf(monkeypatch: pytest.MonkeyPatch) -> None:
    import sys

    monkeypatch.setitem(sys.modules, "pyscf", None)
    case = next(c for c in compare_pyscf.CASES if c.name == "lih-6-31g-rhf")
    row = report.validate_case(case, None, None, ["PySCF unavailable"])
    assert row["molecule"] == "LiH"
    assert (row["input_derived_n_alpha"], row["input_derived_n_beta"]) == (2, 2)
    assert row["status"] == "FAIL"
    json.dumps(row, allow_nan=False)


def test_overlap_rank_change_fails_reference_partition() -> None:
    case = next(c for c in compare_pyscf.CASES if c.name == "lih-6-31g-rhf")
    rustiq = valid_rustiq(case)
    rustiq["calculation"]["hf"]["orthogonalization"].update(
        effective_rank=case.ao_dimension - 1, discarded_directions=1
    )
    row = report.validate_case(case, rustiq, (-75.0, None, None), [], True)
    assert row["status"] == "FAIL"
    assert any("overlap rank" in message for message in row["errors"])


def test_generation_preserves_missing_dependency_failures(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    import sys

    case = compare_pyscf.CASES[0]
    store = tmp_path / "isolated-store"
    store.mkdir()
    monkeypatch.setattr(compare_pyscf, "CASES", [case])
    monkeypatch.setattr(
        compare_pyscf, "prepare_rustiq_env", lambda: {"RUSTIQ_DATA_HOME": str(store)}
    )
    monkeypatch.setattr(compare_pyscf, "rustiq_result", lambda *_: valid_rustiq(case))

    def unavailable(*_):
        raise ImportError("PySCF unavailable")

    monkeypatch.setattr(compare_pyscf, "pyscf_result", unavailable)
    monkeypatch.setattr(report, "metadata", dict)
    monkeypatch.setitem(sys.modules, "integrals", None)
    generated = report.generate(tmp_path / "output")
    assert not generated["passed"]
    assert generated["cases"][0]["case_id"] == case.name
    assert generated["cases"][0]["pyscf_hf_converged"] is None
    assert generated["cases"][0]["status"] == "FAIL"
    assert not generated["integrals"]["passed"]
    assert "PySCF unavailable" in (tmp_path / "output/validation.md").read_text()
