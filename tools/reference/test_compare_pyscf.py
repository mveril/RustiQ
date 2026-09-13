from __future__ import annotations

import shutil
from collections.abc import Iterator
from pathlib import Path
from typing import Any

import compare_pyscf
import pytest


@pytest.fixture(scope="session")
def rustiq_env() -> Iterator[dict[str, str]]:
    env = compare_pyscf.prepare_rustiq_env()
    yield env
    shutil.rmtree(Path(env["RUSTIQ_DATA_HOME"]), ignore_errors=True)


@pytest.mark.parametrize("case", compare_pyscf.CASES, ids=lambda case: case.name)
def test_rustiq_matches_pyscf(
    case: compare_pyscf.ReferenceCase,
    rustiq_env: dict[str, str],
) -> None:
    rustiq = compare_pyscf.rustiq_result(case, rustiq_env)
    pyscf_hf_energy, pyscf_mp2_energy, pyscf_s_squared = compare_pyscf.pyscf_result(case)

    assert rustiq["schema_version"] == 1
    calculation = rustiq["calculation"]
    assert isinstance(calculation, dict)
    hf = calculation["hf"]
    assert isinstance(hf, dict)
    assert hf["converged"] is True
    assert hf["method"] == case.method.upper()
    assert hf["orthogonalization"]["ao_basis_dimension"] == case.ao_dimension
    assert float(hf["total_energy"]) == pytest.approx(
        pyscf_hf_energy, abs=case.tolerance, rel=0.0
    )
    if case.method == "uhf":
        assert pyscf_s_squared is not None
        spin = hf["spin"]
        assert isinstance(spin, dict)
        ideal_s_squared = 0.5 * case.spin * (0.5 * case.spin + 1.0)
        assert float(spin["ideal_s_squared"]) == pytest.approx(ideal_s_squared, abs=1e-12)
        assert float(spin["s_squared"]) == pytest.approx(
            pyscf_s_squared, abs=case.spin_tolerance, rel=0.0
        )
        assert float(spin["spin_contamination"]) == pytest.approx(
            pyscf_s_squared - ideal_s_squared, abs=case.spin_tolerance, rel=0.0
        )
    else:
        assert pyscf_s_squared is None
        assert "spin" not in hf

    mp2 = calculation.get("mp2")
    assert (mp2 is not None) is case.mp2
    assert (pyscf_mp2_energy is not None) is case.mp2
    if case.mp2:
        assert isinstance(mp2, dict)
        assert mp2["method"] == f"{case.method.upper()}-MP2"
        assert case.mp2_tolerance is not None
        assert float(mp2["correlation_energy"]) == pytest.approx(
            pyscf_mp2_energy, abs=case.mp2_tolerance, rel=0.0
        )


def test_rustiq_result_rejects_malformed_json(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def malformed_output(*args: Any, **kwargs: Any) -> str:
        return "not JSON"

    monkeypatch.setattr(compare_pyscf, "run_command", malformed_output)

    with pytest.raises(RuntimeError, match="Could not parse RustiQ JSON output"):
        compare_pyscf.rustiq_result(compare_pyscf.CASES[0], {})
