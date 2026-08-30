#!/usr/bin/env python3
"""Helpers and reference cases for RustiQ/PySCF pytest comparisons.

The script is intended for development checks, not for the Rust test suite.
It prepares an isolated RustiQ basis store from tests/data/sto-3g.json so it
does not need network access for the RustiQ runs.
"""

from __future__ import annotations

import json
import os
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
    mp2: bool = False
    mp2_tolerance: float | None = None


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
        # OH has open-shell orbital near-degeneracies and backend-dependent
        # convergence behavior, so retain its scientifically justified margin.
        tolerance=1e-5,
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
        mp2=True,
        mp2_tolerance=1e-10,
    ),
]


def run_command(args: list[str], *, env: dict[str, str] | None = None) -> str:
    completed = subprocess.run(
        args,
        cwd=REPO_ROOT,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
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
    basis_store = data_home / "RustiQ/basis_sets"
    basis_store.mkdir(parents=True)
    shutil.copyfile(
        REPO_ROOT / "tests/data/sto-3g.json",
        basis_store / "sto-3g.json",
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
    stdout = run_command(
        command,
        env=env,
    )
    try:
        return json.loads(stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError(
            f"Could not parse RustiQ JSON output for {case.name}: {error}\nstdout:\n{stdout}"
        ) from error


def pyscf_result(case: ReferenceCase) -> tuple[float, float | None]:
    from pyscf import gto, mp, scf

    mol = gto.M(
        atom=load_xyz_body(case.xyz),
        basis=case.basis,
        charge=case.charge,
        spin=case.spin,
        unit="Angstrom",
        verbose=0,
    )
    if case.method == "rhf":
        mf = scf.RHF(mol)
    elif case.method == "uhf":
        mf = scf.UHF(mol)
    else:
        raise ValueError(f"Unsupported PySCF method: {case.method}")

    mf.conv_tol = case.conv_tol
    mf.max_cycle = case.max_cycle
    energy = mf.kernel()
    if not mf.converged:
        raise RuntimeError(f"PySCF did not converge for {case.name}.")
    mp2_correlation_energy = None
    if case.mp2:
        if case.method != "rhf":
            raise ValueError(f"MP2 reference is only configured for RHF: {case.name}")
        mp2_correlation_energy, _ = mp.MP2(mf).kernel()
    return float(energy), (
        float(mp2_correlation_energy) if mp2_correlation_energy is not None else None
    )


def main(pytest_args: list[str] | None = None) -> int:
    """Run the canonical pytest reference suite, forwarding any arguments."""
    import pytest

    test_path = Path(__file__).with_name("test_compare_pyscf.py")
    return pytest.main([str(test_path), *(pytest_args or [])])


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
