#!/usr/bin/env python3
"""Helpers and reference cases for RustiQ/PySCF pytest comparisons.

The script is intended for development checks, not for the Rust test suite.
It prepares an isolated store from basis files fetched through RustiQ from BSE.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]


@dataclass(frozen=True)
class ReferenceCase:
    name: str
    runfile: Path
    xyz: Path
    basis: str
    charge: int
    spin: int
    method: str
    conv_tol: float
    max_cycle: int
    tolerance: float
    ao_dimension: int
    spin_tolerance: float = 1e-8
    mp2: bool = False
    mp2_tolerance: float | None = None
    frozen_orbitals: int = 0
    frozen_core_reference: bool = False
    bse_reference: bool = False


class PyscfMp2Error(RuntimeError):
    """MP2 failed after converged HF; retain the completed HF diagnostics."""

    def __init__(
        self,
        case_name: str,
        hf_result: tuple[float, None, float | None],
        cause: Exception,
    ):
        super().__init__(f"PySCF MP2 failed for {case_name}: {cause}")
        self.hf_result = hf_result


class PyscfScfConvergenceError(RuntimeError):
    """PySCF reached its cycle limit without converging HF."""


CASES = [
    ReferenceCase(
        name="h2-sto-3g-rhf",
        runfile=REPO_ROOT / "samples/h2/sto-3g/calculation.toml",
        xyz=REPO_ROOT / "samples/h2/molecule.xyz",
        basis="sto-3g",
        charge=0,
        spin=0,
        method="rhf",
        conv_tol=1e-10,
        max_cycle=80,
        # The JSON contract preserves the Rust f64 rather than the former
        # six-decimal text report. H2 is deterministic and non-degenerate.
        tolerance=2e-10,
        ao_dimension=2,
    ),
    ReferenceCase(
        name="h2-6-31g-rhf",
        runfile=REPO_ROOT / "samples/h2/6-31g/calculation.toml",
        xyz=REPO_ROOT / "samples/h2/molecule.xyz",
        basis="6-31g",
        charge=0,
        spin=0,
        method="rhf",
        conv_tol=1e-10,
        max_cycle=80,
        # The remaining difference is about 3.7e-9 Hartree.
        tolerance=5e-9,
        ao_dimension=4,
    ),
    ReferenceCase(
        name="h2-cc-pvdz-rhf",
        runfile=REPO_ROOT / "samples/h2/cc-pvdz/calculation.toml",
        xyz=REPO_ROOT / "samples/h2/molecule.xyz",
        basis="cc-pvdz",
        charge=0,
        spin=0,
        method="rhf",
        conv_tol=1e-10,
        max_cycle=80,
        tolerance=2e-9,
        ao_dimension=10,
    ),
    ReferenceCase(
        name="h2o-sto-3g-rhf",
        runfile=REPO_ROOT / "samples/h2o/sto-3g/calculation.toml",
        xyz=REPO_ROOT / "samples/h2o/h2o.xyz",
        basis="sto-3g",
        charge=0,
        spin=0,
        method="rhf",
        conv_tol=1e-10,
        max_cycle=80,
        # The BSE coefficients reproduce PySCF within about 2.5e-8 Hartree.
        tolerance=3e-8,
        ao_dimension=7,
    ),
    ReferenceCase(
        name="h2o-6-31g-rhf",
        runfile=REPO_ROOT / "samples/h2o/6-31g/calculation.toml",
        xyz=REPO_ROOT / "samples/h2o/h2o.xyz",
        basis="6-31g",
        charge=0,
        spin=0,
        method="rhf",
        conv_tol=1e-10,
        max_cycle=80,
        # The BSE coefficients reproduce PySCF within about 7e-9 Hartree.
        tolerance=1e-8,
        ao_dimension=13,
    ),
    ReferenceCase(
        name="h2o-cc-pvdz-rhf",
        runfile=REPO_ROOT / "samples/h2o/cc-pvdz/calculation.toml",
        xyz=REPO_ROOT / "samples/h2o/h2o.xyz",
        basis="cc-pvdz",
        charge=0,
        spin=0,
        method="rhf",
        conv_tol=1e-10,
        max_cycle=80,
        tolerance=1e-10,
        ao_dimension=24,
    ),
    ReferenceCase(
        name="h2-plus-sto-3g-uhf",
        runfile=REPO_ROOT / "samples/h2/sto-3g/uhf_h2_plus_calculation.toml",
        xyz=REPO_ROOT / "samples/h2/molecule.xyz",
        basis="sto-3g",
        charge=1,
        spin=1,
        method="uhf",
        conv_tol=1e-10,
        max_cycle=80,
        # H2+ is deterministic and non-degenerate; its UHF implementation
        # difference is about 8e-10 Hartree, still far below the old text limit.
        tolerance=1e-9,
        ao_dimension=2,
    ),
    ReferenceCase(
        name="oh-sto-3g-uhf",
        runfile=REPO_ROOT / "samples/oh/sto-3g/calculation.toml",
        xyz=REPO_ROOT / "samples/oh/oh.xyz",
        basis="sto-3g",
        charge=0,
        spin=1,
        method="uhf",
        conv_tol=1e-5,
        max_cycle=100,
        tolerance=5e-8,
        ao_dimension=6,
        spin_tolerance=2e-5,
    ),
    ReferenceCase(
        name="oh-sto-3g-uhf-mp2",
        runfile=REPO_ROOT / "samples/oh/sto-3g/mp2_calculation.toml",
        xyz=REPO_ROOT / "samples/oh/oh.xyz",
        basis="sto-3g",
        charge=0,
        spin=1,
        method="uhf",
        conv_tol=1e-10,
        max_cycle=100,
        tolerance=5e-8,
        ao_dimension=6,
        spin_tolerance=2e-5,
        mp2=True,
        mp2_tolerance=5e-9,
        frozen_core_reference=True,
    ),
    ReferenceCase(
        name="h2-sto-3g-rhf-mp2",
        runfile=REPO_ROOT / "samples/h2/sto-3g/mp2_calculation.toml",
        xyz=REPO_ROOT / "samples/h2/molecule.xyz",
        basis="sto-3g",
        charge=0,
        spin=0,
        method="rhf",
        conv_tol=1e-10,
        max_cycle=80,
        tolerance=2e-10,
        ao_dimension=2,
        mp2=True,
        mp2_tolerance=1e-10,
    ),
    ReferenceCase(
        name="h2-cc-pvdz-rhf-mp2",
        runfile=REPO_ROOT / "samples/h2/cc-pvdz/mp2_calculation.toml",
        xyz=REPO_ROOT / "samples/h2/molecule.xyz",
        basis="cc-pvdz",
        charge=0,
        spin=0,
        method="rhf",
        conv_tol=1e-12,
        max_cycle=80,
        tolerance=2e-9,
        ao_dimension=10,
        mp2=True,
        mp2_tolerance=1e-9,
    ),
    ReferenceCase(
        name="h2o-6-31g-rhf-mp2",
        runfile=REPO_ROOT / "samples/h2o/6-31g/mp2_calculation.toml",
        xyz=REPO_ROOT / "samples/h2o/h2o.xyz",
        basis="6-31g",
        charge=0,
        spin=0,
        method="rhf",
        conv_tol=1e-12,
        max_cycle=80,
        tolerance=1e-8,
        ao_dimension=13,
        mp2=True,
        # The residual correlation-energy difference is about 5.1e-9 Hartree
        # with the checked-in BSE basis and a 1e-12 SCF threshold.
        mp2_tolerance=1e-8,
        frozen_core_reference=True,
    ),
]

# Tolerances and measured error margins are justified in README.md.
# New beta qualification cases use exact checked-in BSE coefficients, not
# PySCF's similarly named packaged basis. Existing references remain unchanged.
CASES += [
    ReferenceCase(
        name="lih-6-31g-rhf",
        runfile=REPO_ROOT / "samples/lih/6-31g/calculation.toml",
        xyz=REPO_ROOT / "samples/lih/lih.xyz",
        basis="6-31g",
        charge=0,
        spin=0,
        method="rhf",
        conv_tol=1e-12,
        max_cycle=100,
        tolerance=1e-11,
        ao_dimension=11,
        bse_reference=True,
        mp2=False,
        mp2_tolerance=None,
        spin_tolerance=2e-9,
    ),
    ReferenceCase(
        name="co-sto-3g-rhf",
        runfile=REPO_ROOT / "samples/co/sto-3g/calculation.toml",
        xyz=REPO_ROOT / "samples/co/co.xyz",
        basis="sto-3g",
        charge=0,
        spin=0,
        method="rhf",
        conv_tol=1e-12,
        max_cycle=100,
        tolerance=2e-11,
        ao_dimension=10,
        bse_reference=True,
        mp2=False,
        mp2_tolerance=None,
        spin_tolerance=2e-9,
    ),
    ReferenceCase(
        name="co-sto-3g-rhf-mp2",
        runfile=REPO_ROOT / "samples/co/sto-3g/mp2_calculation.toml",
        xyz=REPO_ROOT / "samples/co/co.xyz",
        basis="sto-3g",
        charge=0,
        spin=0,
        method="rhf",
        conv_tol=1e-12,
        max_cycle=100,
        tolerance=2e-11,
        ao_dimension=10,
        bse_reference=True,
        mp2=True,
        mp2_tolerance=2e-11,
        spin_tolerance=2e-9,
        frozen_core_reference=True,
    ),
    ReferenceCase(
        name="h2o-cc-pvdz-rhf-mp2",
        runfile=REPO_ROOT / "samples/h2o/cc-pvdz/mp2_calculation.toml",
        xyz=REPO_ROOT / "samples/h2o/h2o.xyz",
        basis="cc-pvdz",
        charge=0,
        spin=0,
        method="rhf",
        conv_tol=1e-12,
        max_cycle=80,
        tolerance=1e-11,
        ao_dimension=24,
        bse_reference=True,
        mp2=True,
        mp2_tolerance=1e-11,
        spin_tolerance=2e-9,
    ),
    ReferenceCase(
        name="oh-6-31g-uhf",
        runfile=REPO_ROOT / "samples/oh/6-31g/calculation.toml",
        xyz=REPO_ROOT / "samples/oh/oh.xyz",
        basis="6-31g",
        charge=0,
        spin=1,
        method="uhf",
        conv_tol=1e-12,
        max_cycle=100,
        tolerance=2e-11,
        ao_dimension=11,
        bse_reference=True,
        mp2=False,
        mp2_tolerance=None,
        spin_tolerance=2e-9,
    ),
    ReferenceCase(
        name="oh-6-31g-uhf-mp2",
        runfile=REPO_ROOT / "samples/oh/6-31g/mp2_calculation.toml",
        xyz=REPO_ROOT / "samples/oh/oh.xyz",
        basis="6-31g",
        charge=0,
        spin=1,
        method="uhf",
        conv_tol=1e-12,
        max_cycle=100,
        tolerance=2e-11,
        ao_dimension=11,
        bse_reference=True,
        mp2=True,
        mp2_tolerance=2e-11,
        spin_tolerance=2e-9,
        frozen_core_reference=True,
    ),
]

FROZEN_CORE_MEMORY_BUDGETS = ("32 KiB", "512 MiB")
RUSTIQ_SCHEMA_VERSION = 1


def validate_rustiq_result(result: object, case_name: str) -> dict[str, object]:
    """Check the stable JSON envelope consumed by reference validation."""
    if not isinstance(result, dict):
        raise TypeError(f"RustiQ JSON output for {case_name} must be an object")
    version = result.get("schema_version")
    if type(version) is not int or version != RUSTIQ_SCHEMA_VERSION:
        raise ValueError(
            f"Unsupported or missing RustiQ schema_version for {case_name}: {version!r}"
        )
    calculation = result.get("calculation")
    if not isinstance(calculation, dict):
        raise TypeError(
            f"RustiQ JSON output for {case_name} is missing calculation object"
        )
    if not isinstance(calculation.get("hf"), dict):
        raise TypeError(
            f"RustiQ JSON output for {case_name} is missing calculation.hf object"
        )
    return result


def run_command(args: list[str], *, env: dict[str, str] | None = None) -> str:
    completed = subprocess.run(
        args,
        cwd=REPO_ROOT,
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError(
            "Command failed with exit code "
            f"{completed.returncode}: {' '.join(args)}\n"
            f"stdout:\n{completed.stdout}\n"
            f"stderr:\n{completed.stderr}"
        )
    return completed.stdout


def load_xyz_body(path: Path) -> str:
    lines = path.read_text(encoding="utf-8").splitlines()
    return "\n".join(lines[2:])


def prepare_rustiq_env() -> dict[str, str]:
    data_home = Path(tempfile.mkdtemp(prefix="rustiq-reference-"))
    env = os.environ.copy()
    env["RUSTIQ_DATA_HOME"] = str(data_home)
    env["RUSTIQ_AUTO_DOWNLOAD"] = "0"
    # Explicit user overrides must not escape the isolated versioned store.
    env.pop("RUSTIQ_DATA_BASIS", None)
    env["RUSTIQ_CACHE_HOME"] = str(data_home / "cache")
    fixture_store = REPO_ROOT / "tests/data/reference/RustiQ/basis_sets"
    basis_store = data_home / "rustiq/basis_sets"
    basis_store.mkdir(parents=True)
    for basis_name in sorted({case.basis for case in CASES}):
        shutil.copyfile(
            fixture_store / f"{basis_name}.json",
            basis_store / f"{basis_name}.json",
        )
    return env


def rustiq_result(case: ReferenceCase, env: dict[str, str]) -> dict[str, object]:
    rustiq_bin = os.environ.get("RUSTIQ_BIN")
    command = (
        [rustiq_bin, "run", str(case.runfile), "--format", "json"]
        if rustiq_bin
        else [
            "cargo",
            "run",
            "--quiet",
            "--",
            "run",
            str(case.runfile),
            "--format",
            "json",
        ]
    )
    # A unique portable snapshot exposes the actual resolved configuration and
    # geometry without extending the public calculation-result JSON schema.
    with tempfile.TemporaryDirectory(prefix="rustiq-state-") as temp:
        env = dict(env, RUSTIQ_CACHE_HOME=str(Path(temp) / "cache"))
        artifact = Path(temp) / "reference.rustiq"
        if case.bse_reference:
            command += ["--artifact", str(artifact)]
        stdout = run_command(command, env=env)
        resolved = None
        if case.bse_reference:
            prefix = [rustiq_bin] if rustiq_bin else ["cargo", "run", "--quiet", "--"]
            inspection = json.loads(
                run_command(
                    [*prefix, "artifact", "inspect", str(artifact), "--format", "json"],
                    env=env,
                )
            )
            resolved = inspection["calculations"][0]["resolved"]
            validate_resolved(case, resolved)
    try:
        parsed = json.loads(stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError(
            f"Could not parse RustiQ JSON output for {case.name}: {error}\nstdout:\n{stdout}"
        ) from error
    result = validate_rustiq_result(parsed, case.name)
    if resolved is not None:
        result["reference_resolved"] = resolved
    return result


def bse_basis(case: ReferenceCase) -> dict:
    """Interpret raw BSE contractions independently of RustiQ normalization."""
    from pyscf import gto

    source = json.loads(
        (
            REPO_ROOT / "tests/data/reference/RustiQ/basis_sets" / f"{case.basis}.json"
        ).read_text()
    )
    basis = {}
    symbols = {
        line.split()[0] for line in load_xyz_body(case.xyz).splitlines() if line.strip()
    }
    for symbol in symbols:
        z = gto.charge(symbol)
        shells = []
        for shell in source["elements"][str(z)]["electron_shells"]:
            angular = shell["angular_momentum"]
            coefficients = shell["coefficients"]
            if len(angular) == 1:
                angular = angular * len(coefficients)
            if len(angular) != len(coefficients):
                raise ValueError("Unsupported BSE contraction layout")
            for l, coeff in zip(angular, coefficients, strict=True):
                shells.append(
                    [
                        l,
                        *[
                            [float(a), float(c)]
                            for a, c in zip(shell["exponents"], coeff, strict=True)
                        ],
                    ]
                )
        basis[symbol] = shells
    return basis


def electron_counts(case: ReferenceCase) -> tuple[int, int]:
    # This metadata must remain available when PySCF itself fails to import.
    charges = {"H": 1, "Li": 3, "C": 6, "O": 8}
    symbols = [
        line.split()[0] for line in load_xyz_body(case.xyz).splitlines() if line.strip()
    ]
    electrons = (
        sum(int(symbol) if symbol.isdigit() else charges[symbol] for symbol in symbols)
        - case.charge
    )
    if (electrons - case.spin) % 2 or electrons < case.spin:
        raise ValueError("Inconsistent charge and spin")
    return (electrons + case.spin) // 2, (electrons - case.spin) // 2


def validate_resolved(case: ReferenceCase, resolved: dict) -> None:
    import tomllib

    import numpy as np

    config = tomllib.loads(resolved["configuration"])
    molecule = config["molecule"]
    assert molecule["charge"] == case.charge
    assert molecule["multiplicity"] == case.spin + 1
    assert molecule["units"] == "Bohr"
    assert config["basis"]["name"] == case.basis
    hf = config["method"]["hf"]
    assert hf["method"].lower() == case.method
    assert hf["convergence_threshold"] == case.conv_tol
    assert hf["max_iterations"] == case.max_cycle
    if case.name.startswith("oh-6-31g"):
        assert hf["guess"] == {
            "type": "Random",
            "distribution": "Normal",
            "mean": 0.0,
            "std_dev": 1.0,
            "seed": 42,
        }
    if case.mp2:
        assert config["method"]["mp2"]["frozen_orbitals"] == case.frozen_orbitals
    expected = [
        line.split() for line in load_xyz_body(case.xyz).splitlines() if line.strip()
    ]
    actual = [
        line.split() for line in resolved["geometry"].splitlines()[2:] if line.strip()
    ]
    assert [row[0] for row in actual] == [row[0] for row in expected]
    np.testing.assert_allclose(
        [[float(x) for x in row[1:]] for row in actual],
        [[float(x) / 0.529177210903 for x in row[1:]] for row in expected],
        atol=1e-13,
        rtol=0,
    )


def pyscf_result(
    case: ReferenceCase, diagnostics: dict | None = None
) -> tuple[float, float | None, float | None]:
    from pyscf import gto, mp, scf

    atoms = load_xyz_body(case.xyz)
    if case.bse_reference:
        atoms = [
            (row.split()[0], [float(x) / 0.529177210903 for x in row.split()[1:]])
            for row in atoms.splitlines()
            if row.strip()
        ]
    mol = gto.M(
        atom=atoms,
        basis=bse_basis(case) if case.bse_reference else case.basis,
        charge=case.charge,
        spin=case.spin,
        unit="Bohr" if case.bse_reference else "Angstrom",
        cart=False,
        verbose=0,
    )
    assert mol.nao_nr() == case.ao_dimension
    assert mol.nelec == electron_counts(case)
    if case.method == "rhf":
        mf = scf.RHF(mol)
    elif case.method == "uhf":
        mf = scf.UHF(mol)
    else:
        raise ValueError(f"Unsupported PySCF method: {case.method}")

    if case.bse_reference:
        mf.conv_tol_grad = 1e-10
        mf.init_guess = "1e"
    mf.conv_tol = case.conv_tol
    mf.max_cycle = case.max_cycle
    energy = mf.kernel()
    if diagnostics is not None:
        diagnostics.update(
            last_hf_total_energy_hartree=float(energy),
            iterations=mf.cycles,
            converged=bool(mf.converged),
            n_alpha=mol.nelec[0],
            n_beta=mol.nelec[1],
            virtual_alpha=mol.nao_nr() - mol.nelec[0],
            virtual_beta=mol.nao_nr() - mol.nelec[1],
        )
    if not mf.converged:
        raise PyscfScfConvergenceError(f"PySCF did not converge for {case.name}.")
    s_squared = float(mf.spin_square()[0]) if case.method == "uhf" else None
    mp2_correlation_energy = None
    if case.mp2:
        try:
            mp2_correlation_energy, _ = mp.MP2(mf, frozen=case.frozen_orbitals).kernel()
        except Exception as error:
            raise PyscfMp2Error(
                case.name, (float(energy), None, s_squared), error
            ) from error
    return (
        float(energy),
        (float(mp2_correlation_energy) if mp2_correlation_energy is not None else None),
        s_squared,
    )


def frozen_core_case(
    case: ReferenceCase, budget: str, directory: Path
) -> ReferenceCase:
    """Construct the frozen-core variants shared by pytest and reporting."""
    source = case.runfile.read_text(encoding="utf-8")
    source = re.sub(
        r"^geometry = .*$",
        lambda _: f"geometry = {json.dumps(str(case.xyz))}",
        source,
        flags=re.MULTILINE,
    )
    marker = "frozen_orbitals = 0"
    if source.count(marker) != 1:
        raise ValueError(f"Expected one MP2 frozen-orbital setting in {case.runfile}")
    source = source.replace(marker, f'frozen_orbitals = 1\nmemory_limit = "{budget}"')
    directory.mkdir(parents=True, exist_ok=True)
    runfile = directory / f"{case.name}-frozen-mp2-{budget.replace(' ', '-')}.toml"
    runfile.write_text(source, encoding="utf-8")
    import tomllib

    generated = tomllib.loads(source)
    mp2 = generated.get("method", {}).get("mp2", {})
    if mp2.get("frozen_orbitals") != 1 or mp2.get("memory_limit") != budget:
        raise ValueError(
            "Generated frozen-core runfile did not apply its requested settings"
        )
    from dataclasses import replace

    return replace(case, runfile=runfile, frozen_orbitals=1)


def main(pytest_args: list[str] | None = None) -> int:
    """Run the canonical pytest reference suite, forwarding any arguments."""
    import pytest

    test_paths = [
        Path(__file__).with_name("test_compare_pyscf.py"),
        Path(__file__).with_name("test_compare_pyscf_helpers.py"),
    ]
    return pytest.main([*(str(path) for path in test_paths), *(pytest_args or [])])


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
