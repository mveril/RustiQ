from __future__ import annotations

import json
import math
from pathlib import Path

import compare_pyscf
import report


def test_hf_mp2_and_spin_differences_and_markdown() -> None:
    case = next(c for c in compare_pyscf.CASES if c.name == "oh-sto-3g-uhf-mp2")
    rustiq = {"calculation": {"hf": {"converged": True, "total_energy": -75.0,
        "method": "UHF", "orthogonalization": {"ao_basis_dimension": case.ao_dimension},
        "spin": {"s_squared": 0.752}}, "mp2": {"method": "UHF-MP2",
        "correlation_energy": -0.2}}}
    result = report.validate_case(case, rustiq, (-75.0 + 1e-9, -0.2 + 1e-10, 0.752 + 1e-6), [])
    assert math.isclose(result["hf_absolute_difference_hartree"], 1e-9, abs_tol=1e-12)
    assert math.isclose(result["mp2_absolute_difference_hartree"], 1e-10, abs_tol=1e-12)
    assert math.isclose(result["s_squared_absolute_difference"], 1e-6, abs_tol=1e-12)
    assert result["status"] == "PASS"
    md = report.render_markdown({"cases": [result], "metadata": {}})
    assert "total molecular HF energies" in md
    assert "correlation energies only" in md


def test_missing_and_non_finite_values_fail() -> None:
    case = compare_pyscf.CASES[0]
    result = report.validate_case(case, {"calculation": {"hf": {"converged": True,
        "total_energy": float("nan"), "method": "RHF",
        "orthogonalization": {"ao_basis_dimension": case.ao_dimension}}}}, (float("inf"), None, None), [])
    assert result["rustiq_hf_total_energy_hartree"] is None
    assert result["pyscf_hf_total_energy_hartree"] is None
    assert result["status"] == "FAIL"
    missing = report.validate_case(case, None, None, ["calculation failed"])
    assert missing["status"] == "FAIL"
    assert missing["errors"]


def test_failed_tolerance_is_reported() -> None:
    case = compare_pyscf.CASES[0]
    row = report.validate_case(case, {"calculation": {"hf": {"converged": True,
        "total_energy": -1.0, "method": "RHF",
        "orthogonalization": {"ao_basis_dimension": case.ao_dimension}}}}, (-0.9, None, None), [])
    assert not row["hf_pass"]
    assert row["status"] == "FAIL"


def test_json_serialization_preserves_full_precision_and_rejects_nan(tmp_path: Path) -> None:
    payload = {"energy": -1.2345678901234567}
    path = tmp_path / "report.json"
    path.write_text(json.dumps(payload, allow_nan=False), encoding="utf-8")
    assert json.loads(path.read_text())["energy"] == payload["energy"]


def test_markdown_scientific_notation_and_failure_detail() -> None:
    item = {"case_id": "failed", "molecule": "H2", "geometry_path": "samples/h2/molecule.xyz",
        "basis": "sto-3g", "charge": 0, "spin_n_alpha_minus_n_beta": 0,
        "ao_basis_dimension": 2, "hf_method": "RHF", "rustiq_hf_total_energy_hartree": None,
        "pyscf_hf_total_energy_hartree": None, "hf_absolute_difference_hartree": None,
        "hf_tolerance_hartree": 1e-10, "hf_pass": False, "status": "FAIL",
        "errors": ["HF energy comparison failed"]}
    md = report.render_markdown({"cases": [item], "metadata": {}})
    assert "0/1 cases passed" in md
    assert "HF energy comparison failed" in md
    assert "—" in md


def test_molecular_formula_uses_geometry_atoms() -> None:
    case = next(c for c in compare_pyscf.CASES if c.name == "h2o-sto-3g-rhf")
    assert report.molecular_formula(case.xyz) == "H2O"
    h2_case = compare_pyscf.CASES[0]
    assert report.molecular_formula(h2_case.xyz) == "H2"
