from __future__ import annotations

import json
import math
import shutil
from collections.abc import Iterator
from pathlib import Path

import compare_pyscf
import pytest


pytest.importorskip("pyscf", reason="PySCF is not installed in this environment")


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
    diagnostics: dict = {}
    pyscf_hf_energy, pyscf_mp2_energy, pyscf_s_squared = compare_pyscf.pyscf_result(
        case, diagnostics
    )
    assert diagnostics["converged"] is True
    assert (
        diagnostics["n_alpha"],
        diagnostics["n_beta"],
    ) == compare_pyscf.electron_counts(case)
    assert math.isfinite(pyscf_hf_energy)

    assert rustiq["schema_version"] == 1
    calculation = rustiq["calculation"]
    assert isinstance(calculation, dict)
    hf = calculation["hf"]
    assert isinstance(hf, dict)
    assert hf["converged"] is True
    assert hf["method"] == case.method.upper()
    assert math.isfinite(hf["total_energy"])
    if case.bse_reference:
        assert hf["orthogonalization"]["effective_rank"] == case.ao_dimension
        assert hf["orthogonalization"]["discarded_directions"] == 0
    assert hf["orthogonalization"]["ao_basis_dimension"] == case.ao_dimension
    assert float(hf["total_energy"]) == pytest.approx(
        pyscf_hf_energy, abs=case.tolerance, rel=0.0
    )
    if case.method == "uhf":
        assert pyscf_s_squared is not None
        spin = hf["spin"]
        assert isinstance(spin, dict)
        ideal_s_squared = 0.5 * case.spin * (0.5 * case.spin + 1.0)
        assert float(spin["ideal_s_squared"]) == pytest.approx(
            ideal_s_squared, abs=1e-12, rel=0
        )
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
        assert math.isfinite(mp2["correlation_energy"])
        assert math.isfinite(pyscf_mp2_energy)
        assert mp2["total_energy"] == pytest.approx(
            hf["total_energy"] + mp2["correlation_energy"], abs=1e-12, rel=0
        )
        assert mp2["electronic_energy"] == pytest.approx(
            hf["electronic_energy"] + mp2["correlation_energy"], abs=1e-12, rel=0
        )
        assert mp2["total_energy"] == pytest.approx(
            pyscf_hf_energy + pyscf_mp2_energy,
            abs=case.tolerance + case.mp2_tolerance,
            rel=0,
        )
        assert float(mp2["correlation_energy"]) == pytest.approx(
            pyscf_mp2_energy, abs=case.mp2_tolerance, rel=0.0
        )


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

    with pytest.raises(
        (TypeError, ValueError), match="RustiQ JSON output|schema_version"
    ):
        compare_pyscf.rustiq_result(compare_pyscf.CASES[0], {})


@pytest.mark.parametrize(
    "name", [case.name for case in compare_pyscf.CASES if case.frozen_core_reference]
)
@pytest.mark.parametrize("budget", compare_pyscf.FROZEN_CORE_MEMORY_BUDGETS)
def test_frozen_core_mp2_matches_pyscf(
    name: str,
    budget: str,
    tmp_path: Path,
    rustiq_env: dict[str, str],
) -> None:
    original = next(case for case in compare_pyscf.CASES if case.name == name)
    case = compare_pyscf.frozen_core_case(original, budget, tmp_path)
    test_rustiq_matches_pyscf(case, rustiq_env)


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
    config = tomllib.loads(case.runfile.read_text())
    assert compare_pyscf.electron_counts(case) == counts
    assert case.bse_reference
    assert case.frozen_orbitals == 0
    assert min(counts) > 0 and max(counts) < case.ao_dimension
    assert config["method"]["hf"]["convergence_threshold"] == case.conv_tol
    assert config["method"]["hf"]["max_iterations"] == case.max_cycle
    if case.mp2:
        assert config["method"]["mp2"]["frozen_orbitals"] == 0


@pytest.mark.parametrize("name", ["co-sto-3g-rhf-mp2", "oh-6-31g-uhf-mp2"])
def test_workspace_budgets_select_different_blocks(
    name: str,
    tmp_path: Path,
    rustiq_env: dict[str, str],
) -> None:
    import os
    import re

    original = next(case for case in compare_pyscf.CASES if case.name == name)
    results = []
    for budget in compare_pyscf.FROZEN_CORE_MEMORY_BUDGETS:
        case = compare_pyscf.frozen_core_case(original, budget, tmp_path)
        # The existing text observer reports the actual planner choices. Result
        # JSON intentionally contains no resource diagnostics.
        binary = os.environ.get("RUSTIQ_BIN")
        prefix = [binary] if binary else ["cargo", "run", "--quiet", "--"]
        text = compare_pyscf.run_command(
            [*prefix, "run", str(case.runfile)], env=dict(rustiq_env, NO_COLOR="1")
        )
        plans = re.findall(r"occupied (\d+)/(\d+), virtual \d+/\d+, block (\d+);", text)
        assert len(plans) == (1 if case.method == "rhf" else 3), text
        for left, _, block in plans:
            if budget == "32 KiB":
                assert int(block) == 2 < int(left)
            else:
                assert int(block) == int(left)
        result = compare_pyscf.rustiq_result(case, rustiq_env)
        results.append(result["calculation"]["mp2"]["correlation_energy"])
    # Each budget is also independently compared with PySCF by the canonical
    # frozen-core parametrization above; this checks the two Rust executions.
    assert results[0] == pytest.approx(results[1], abs=1e-12, rel=0)
