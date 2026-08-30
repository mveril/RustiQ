#!/usr/bin/env python3
"""Compare RustiQ sample energies against PySCF references.

The script is intended for development checks, not for the Rust test suite.
It prepares an isolated RustiQ basis store from tests/data/sto-3g.json so it
does not need network access for the RustiQ runs.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

from pyscf import gto, mp, scf


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
            "cargo", "run", "--quiet", "--", "run", str(case.runfile), "--format", "json"
        ]
    )
    stdout = run_command(
        command,
        env=env,
    )
    try:
        result = json.loads(stdout)
        if result["schema_version"] != 1:
            raise RuntimeError(f"Unsupported RustiQ JSON schema for {case.name}.")
        return result
    except (json.JSONDecodeError, KeyError, TypeError) as error:
        raise RuntimeError(
            f"Could not parse RustiQ JSON output for {case.name}: {error}\nstdout:\n{stdout}"
        ) from error


def pyscf_result(case: ReferenceCase) -> tuple[float, float | None]:
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
        print("case,rustiq_hf,pyscf_hf,hf_delta,rustiq_mp2_corr,pyscf_mp2_corr,mp2_delta,status")
        for case in selected_cases(args.cases):
            rustiq = rustiq_result(case, env)
            rustiq_hf = float(rustiq["calculation"]["hf"]["total_energy"])
            pyscf_hf, pyscf_mp2_corr = pyscf_result(case)
            hf_delta = abs(rustiq_hf - pyscf_hf)
            rustiq_mp2 = rustiq["calculation"].get("mp2")
            rustiq_mp2_corr = (
                float(rustiq_mp2["correlation_energy"]) if rustiq_mp2 is not None else None
            )
            if case.mp2 and rustiq_mp2_corr is None:
                raise RuntimeError(f"RustiQ JSON output did not include MP2 for {case.name}.")
            mp2_delta = (
                abs(rustiq_mp2_corr - pyscf_mp2_corr)
                if rustiq_mp2_corr is not None and pyscf_mp2_corr is not None
                else None
            )
            ok = hf_delta <= case.tolerance and (
                mp2_delta is None or mp2_delta <= case.mp2_tolerance
            )
            failures += 0 if ok else 1
            status = "ok" if ok else "failed"
            print(
                f"{case.name},"
                f"{rustiq_hf:.15f},"
                f"{pyscf_hf:.15f},"
                f"{hf_delta:.3e},"
                f"{'' if rustiq_mp2_corr is None else f'{rustiq_mp2_corr:.15f}'},"
                f"{'' if pyscf_mp2_corr is None else f'{pyscf_mp2_corr:.15f}'},"
                f"{'' if mp2_delta is None else f'{mp2_delta:.3e}'},"
                f"{status}"
            )
    finally:
        shutil.rmtree(data_home, ignore_errors=True)

    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
