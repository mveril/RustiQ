#!/usr/bin/env python3
"""Compare RustiQ sample energies against PySCF references.

The script is intended for development checks, not for the Rust test suite.
It prepares an isolated RustiQ basis store from tests/data/sto-3g.json so it
does not need network access for the RustiQ runs.
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

from pyscf import gto, scf


REPO_ROOT = Path(__file__).resolve().parents[2]
TOTAL_ENERGY_RE = re.compile(
    r"Total Energy \(including nuclear repulsion\):\s+([-+0-9.eE]+)\s+Hartree"
)


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
        tolerance=1e-6,
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
        tolerance=1e-5,
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


def rustiq_total_energy(case: ReferenceCase, env: dict[str, str]) -> float:
    rustiq_bin = os.environ.get("RUSTIQ_BIN")
    command = (
        [rustiq_bin, "run", str(case.runfile)]
        if rustiq_bin
        else ["cargo", "run", "--quiet", "--", "run", str(case.runfile)]
    )
    stdout = run_command(
        command,
        env=env,
    )
    match = TOTAL_ENERGY_RE.search(stdout)
    if match is None:
        raise RuntimeError(f"Could not find total energy in RustiQ output for {case.name}.")
    return float(match.group(1))


def pyscf_total_energy(case: ReferenceCase) -> float:
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
    return float(energy)


def selected_cases(names: list[str]) -> list[ReferenceCase]:
    if not names:
        return CASES
    by_name = {case.name: case for case in CASES}
    unknown = sorted(set(names) - set(by_name))
    if unknown:
        raise ValueError(f"Unknown case(s): {', '.join(unknown)}")
    return [by_name[name] for name in names]


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Compare RustiQ sample total energies against PySCF."
    )
    parser.add_argument(
        "cases",
        nargs="*",
        help="Optional case names. Defaults to all cases.",
    )
    args = parser.parse_args()

    env = prepare_rustiq_env()
    data_home = Path(env["RUSTIQ_DATA_HOME"])
    failures = 0

    try:
        print("case,rustiq_hartree,pyscf_hartree,abs_delta,tolerance,status")
        for case in selected_cases(args.cases):
            rustiq_energy = rustiq_total_energy(case, env)
            pyscf_energy = pyscf_total_energy(case)
            delta = abs(rustiq_energy - pyscf_energy)
            ok = delta <= case.tolerance
            failures += 0 if ok else 1
            status = "ok" if ok else "failed"
            print(
                f"{case.name},"
                f"{rustiq_energy:.12f},"
                f"{pyscf_energy:.12f},"
                f"{delta:.3e},"
                f"{case.tolerance:.3e},"
                f"{status}"
            )
    finally:
        shutil.rmtree(data_home, ignore_errors=True)

    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
