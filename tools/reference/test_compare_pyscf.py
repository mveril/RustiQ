from __future__ import annotations

import json
import math
import os
import shutil
from collections.abc import Iterator
from pathlib import Path

import compare_pyscf
import pytest

pytestmark = pytest.mark.requires_rust


@pytest.fixture(scope="session")
def rustiq_env() -> Iterator[dict[str, str]]:
    pytest.importorskip("pyscf", reason="scientific comparisons require PySCF")
    env = compare_pyscf.prepare_rustiq_env()
    yield env
    shutil.rmtree(Path(env["RUSTIQ_DATA_HOME"]), ignore_errors=True)


@pytest.fixture
def rustiq_env_without_pyscf() -> Iterator[dict[str, str]]:
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


@pytest.mark.parametrize("name", ["co-sto-3g-rhf-mp2", "oh-6-31g-uhf-mp2"])
@pytest.mark.requires_rust
def test_workspace_budgets_select_different_blocks(
    name: str,
    tmp_path: Path,
    rustiq_env_without_pyscf: dict[str, str],
) -> None:
    import os
    import re

    original = next(case for case in compare_pyscf.CASES if case.name == name)
    rustiq_env = rustiq_env_without_pyscf
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


@pytest.mark.parametrize("name", ["co-sto-3g-rhf-mp2", "oh-6-31g-uhf-mp2"])
def test_restored_frozen_core_mp2_matches_fresh_and_pyscf(
    name: str,
    tmp_path: Path,
    rustiq_env: dict[str, str],
) -> None:
    original = next(case for case in compare_pyscf.CASES if case.name == name)
    budget = "32 KiB"
    case = compare_pyscf.frozen_core_case(original, budget, tmp_path)
    command = (
        [os.environ["RUSTIQ_BIN"]]
        if "RUSTIQ_BIN" in os.environ
        else ["cargo", "run", "--quiet", "--"]
    )
    artifact = tmp_path / "reference.rustiq"
    fresh = json.loads(
        compare_pyscf.run_command(
            [
                *command,
                "run",
                str(case.runfile),
                "--artifact",
                str(artifact),
                "--format",
                "json",
            ],
            env=rustiq_env,
        )
    )["calculation"]["mp2"]
    settings = tmp_path / "postscf.toml"
    settings.write_text(
        f'[method.mp2]\nfrozen_orbitals = {case.frozen_orbitals}\nmemory_limit = "{budget}"\n',
        encoding="utf-8",
    )
    report = json.loads(
        compare_pyscf.run_command(
            [
                *command,
                "run",
                str(artifact),
                "--post-scf",
                str(settings),
                "--format",
                "json",
            ],
            env=rustiq_env,
        )
    )
    assert report["summary"] == {"succeeded": 1, "skipped": 0, "failed": 0}
    restored = report["calculations"][0]["methods"][0]["result"]
    assert restored["reference"] == case.method.upper()
    for field in ("correlation_energy", "electronic_energy", "total_energy"):
        assert restored[field] == pytest.approx(fresh[field], abs=1e-12, rel=0)
    hf, correlation, _ = compare_pyscf.pyscf_result(case)
    assert correlation is not None
    assert case.mp2_tolerance is not None
    assert restored["correlation_energy"] == pytest.approx(
        correlation, abs=case.mp2_tolerance, rel=0
    )
    assert restored["total_energy"] == pytest.approx(
        hf + correlation, abs=case.tolerance + case.mp2_tolerance, rel=0
    )


@pytest.mark.parametrize(
    "name",
    [
        "h2-sto-3g-rhf",
        "h2o-cc-pvdz-rhf",
        "oh-sto-3g-uhf",
        "h2-plus-sto-3g-uhf",
        2,
        3,
        4,
        "mixed-d",
    ],
)
def test_molden_export_matches_pyscf(
    name: str | int, tmp_path: Path, rustiq_env: dict[str, str]
) -> None:
    import io
    import json
    import os
    import subprocess
    import zipfile

    import numpy as np
    from pyscf import scf
    from pyscf.tools import molden

    if isinstance(name, int) or name == "mixed-d":
        basis = json.loads(
            (compare_pyscf.REPO_ROOT / "tests/data/sto-3g.json").read_text()
        )
        helium = basis["elements"]["1"]
        angular = name if isinstance(name, int) else 2
        helium["electron_shells"].append(
            {
                "function_type": "gto_cartesian",
                "region": "",
                "angular_momentum": [angular],
                "exponents": ["1.2", "0.35"],
                "coefficients": [["0.7", "0.4"]],
            }
        )
        if name == "mixed-d":
            helium["electron_shells"].append(
                dict(helium["electron_shells"][-1], function_type="gto_spherical")
            )
        basis["elements"] = {"2": helium}
        basis_name = f"molden-cart-{name}"
        store = Path(rustiq_env["RUSTIQ_DATA_HOME"]) / "rustiq/basis_sets"
        (store / f"{basis_name}.json").write_text(json.dumps(basis))
        (tmp_path / "molecule.xyz").write_text("1\nHe\nHe 0 0 0\n")
        runfile = tmp_path / "calculation.toml"
        runfile.write_text(f'[basis]\nname = "{basis_name}"\n')
        charge = 0
        ao_dimension = 1 + (angular + 1) * (angular + 2) // 2
        electron_counts = (1, 1)
        unrestricted = False
    else:
        case = next(case for case in compare_pyscf.CASES if case.name == name)
        runfile = case.runfile
        charge = case.charge
        ao_dimension = case.ao_dimension
        electron_counts = compare_pyscf.electron_counts(case)
        unrestricted = case.method == "uhf"

    binary = os.environ.get("RUSTIQ_BIN")
    prefix = [binary] if binary else ["cargo", "run", "--quiet", "--"]
    bundle = tmp_path / "calculation.rustiq"
    output = compare_pyscf.run_command(
        [
            *prefix,
            "run",
            str(runfile),
            "--artifact",
            str(bundle),
            "--format",
            "json",
        ],
        env=rustiq_env,
    )
    hf = json.loads(output)["calculation"]["hf"]
    assert hf["converged"]
    filename = tmp_path / "orbitals.molden"
    export_args = [
        *prefix,
        "artifact",
        "export",
        str(bundle),
        "--format",
        "molden",
        "--calculation",
        "0",
        "-o",
        str(filename),
    ]
    if name == "mixed-d":
        failed = subprocess.run(
            export_args, check=False, env=rustiq_env, capture_output=True, text=True
        )
        assert failed.returncode == 1
        assert "mixed Cartesian and spherical representations" in failed.stderr
        assert not filename.exists()
        return
    stdout = compare_pyscf.run_command(export_args, env=rustiq_env)
    assert stdout == ""
    mol, energies, coefficients, occupations, _, _ = molden.load(filename)
    mol.charge = charge  # Molden encodes electron counts through MO occupations.
    assert mol.nao_nr() == ao_dimension
    assert mol.nelec == electron_counts

    overlap = mol.intor_symmetric("int1e_ovlp")
    channels = (
        zip(energies, coefficients, occupations, strict=True)
        if unrestricted
        else [(energies, coefficients, occupations)]
    )
    for index, (orbital_energies, orbitals, occupation) in enumerate(channels):
        count = mol.nelec[index]
        np.testing.assert_array_equal(
            occupation,
            np.r_[
                np.full(count, 1 if unrestricted else 2),
                np.zeros(len(occupation) - count),
            ],
        )
        np.testing.assert_allclose(
            orbitals.T @ overlap @ orbitals,
            np.eye(orbitals.shape[1]),
            atol=1e-10,
            rtol=0,
        )
        assert np.isfinite(orbital_energies).all()

    # Evaluate the stored and imported basis functions independently at off-axis
    # points. This catches AO permutations and phases even for virtual orbitals.
    points = np.array([[0.28, -0.35, 0.93], [1.11, 0.62, -0.74], [-0.85, 0.41, 1.52]])
    with zipfile.ZipFile(bundle) as archive:
        entry = "calculations/calculation-0"
        context = json.loads(archive.read(f"{entry}/calculation.json"))
        ao_values = []
        for ao in context["basis"]:
            delta = points - ao["center"]
            radius_squared = (delta**2).sum(axis=1)
            value = np.zeros(len(points))
            for component in ao["components"]:
                angular = np.prod(delta ** component["angular_momentum"], axis=1)
                for primitive in component["primitives"]:
                    value += (
                        angular
                        * primitive["coefficient"]
                        * np.exp(-primitive["exponent"] * radius_squared)
                    )
            ao_values.append(value)
        stored_ao = np.array(ao_values).T
        imported_ao = mol.eval_gto("GTOval_cart" if mol.cart else "GTOval_sph", points)
        paths = (
            ["coefficients/alpha.npy", "coefficients/beta.npy"]
            if unrestricted
            else ["coefficients.npy"]
        )
        energy_paths = (
            ["energies/alpha.npy", "energies/beta.npy"]
            if unrestricted
            else ["energies.npy"]
        )
        imported_channels = coefficients if unrestricted else (coefficients,)
        imported_energies = energies if unrestricted else (energies,)
        for path, energy_path, imported, orbital_energies in zip(
            paths, energy_paths, imported_channels, imported_energies, strict=True
        ):
            stored = np.load(io.BytesIO(archive.read(f"{entry}/orbitals/{path}")))
            stored_energies = np.load(
                io.BytesIO(archive.read(f"{entry}/orbitals/{energy_path}"))
            )
            np.testing.assert_array_equal(orbital_energies, stored_energies)
            np.testing.assert_allclose(
                imported_ao @ imported, stored_ao @ stored, atol=1e-10, rtol=0
            )

    # Evaluate the exported density directly, then compare with an independent SCF.
    mf = scf.UHF(mol) if unrestricted else scf.RHF(mol)
    density = mf.make_rdm1(coefficients, occupations)
    reconstructed = mf.energy_tot(dm=density)
    assert reconstructed == pytest.approx(hf["total_energy"], abs=1e-8, rel=0)
    if isinstance(name, int):
        assert reconstructed == pytest.approx(mf.kernel(), abs=1e-8, rel=0)
        assert mf.converged
    else:
        reference, _, _ = compare_pyscf.pyscf_result(case)
        assert reconstructed == pytest.approx(reference, abs=case.tolerance, rel=0)
