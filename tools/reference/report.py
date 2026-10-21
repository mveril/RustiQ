#!/usr/bin/env python3
"""Generate a full precision RustiQ/PySCF scientific validation report."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import platform
import shutil
import subprocess
import tomllib
from dataclasses import replace
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

import compare_pyscf

REPO_ROOT = compare_pyscf.REPO_ROOT


def frozen_runfile(
    case: compare_pyscf.ReferenceCase, budget: str, directory: Path
) -> compare_pyscf.ReferenceCase:
    """Use the same frozen-core runfile construction as the pytest suite."""
    return compare_pyscf.frozen_core_case(case, budget, directory)


def number(value: Any) -> float | None:
    if isinstance(value, bool):
        return None
    try:
        result = float(value)
    except (TypeError, ValueError):
        return None
    return result if math.isfinite(result) else None


def difference(left: Any, right: Any) -> float | None:
    a, b = number(left), number(right)
    return abs(a - b) if a is not None and b is not None else None


def passed(error: float | None, tolerance: float | None, *required: Any) -> bool:
    return (
        all(number(value) is not None for value in required)
        and error is not None
        and tolerance is not None
        and error <= tolerance
    )


def validate_case(
    case: compare_pyscf.ReferenceCase,
    rustiq: dict[str, Any] | None,
    pyscf: tuple[float, float | None, float | None] | None,
    errors: list[str],
    pyscf_hf_converged: bool | None = None,
    pyscf_diagnostics: dict | None = None,
) -> dict[str, Any]:
    if rustiq is not None:
        try:
            rustiq = compare_pyscf.validate_rustiq_result(rustiq, case.name)
        except (TypeError, ValueError) as error:
            errors.append(str(error))
            rustiq = None
    calc: dict[str, Any] = {}
    hf: dict[str, Any] = {}
    mp2: dict[str, Any] | None = None
    spin: dict[str, Any] | None = None
    try:
        calc = rustiq["calculation"] if rustiq is not None else {}
        hf = calc["hf"]
        if not isinstance(hf, dict):
            hf = {}
    except (KeyError, TypeError):
        errors.append("RustiQ result is missing calculation.hf")
    rhf_energy = number(hf.get("total_energy"))
    py_hf = number(pyscf[0]) if pyscf else None
    error_hf = difference(rhf_energy, py_hf)
    converged = hf.get("converged") is True
    hf_method = hf.get("method")
    orthogonalization = hf.get("orthogonalization")
    ao_dimension = (
        orthogonalization.get("ao_basis_dimension")
        if isinstance(orthogonalization, dict)
        else None
    )
    method_matches = hf_method == case.method.upper()
    dimension_matches = ao_dimension == case.ao_dimension
    if not method_matches:
        errors.append("RustiQ HF method does not match the reference case")
    if not dimension_matches:
        errors.append("RustiQ AO basis dimension does not match the reference case")
    if (
        case.bse_reference
        and isinstance(orthogonalization, dict)
        and (
            orthogonalization.get("effective_rank") != case.ao_dimension
            or orthogonalization.get("discarded_directions") != 0
        )
    ):
        dimension_matches = False
        errors.append("RustiQ overlap rank changed the expected occupied/virtual space")
    py_converged = pyscf_hf_converged is True
    hf_ok = (
        converged
        and py_converged
        and method_matches
        and dimension_matches
        and passed(error_hf, case.tolerance, rhf_energy, py_hf)
    )
    if not converged:
        errors.append("RustiQ HF did not converge")
    if pyscf_hf_converged is not True:
        errors.append(
            "PySCF HF did not converge"
            if pyscf_hf_converged is False
            else "PySCF HF convergence status is unknown"
        )
    if py_hf is None:
        errors.append(
            "PySCF HF calculation did not return a finite energy; convergence could not be established"
        )
    if error_hf is None or (error_hf is not None and error_hf > case.tolerance):
        errors.append("HF energy comparison failed")
    result: dict[str, Any] = {
        "case_id": case.name,
        "molecule": molecular_formula(case.xyz),
        "geometry_path": str(case.xyz.relative_to(REPO_ROOT)),
        "basis": case.basis,
        "geometry_provenance": case.xyz.read_text().splitlines()[1],
        "data_sources": "DATA_SOURCES.md",
        "geometry_sha256": hashlib.sha256(case.xyz.read_bytes()).hexdigest(),
        "rustiq_basis_sha256": hashlib.sha256(
            (
                REPO_ROOT
                / "tests/data/reference/RustiQ/basis_sets"
                / f"{case.basis}.json"
            ).read_bytes()
        ).hexdigest(),
        "basis_reference_source": "raw checked-in BSE"
        if case.bse_reference
        else "PySCF packaged basis",
        "ao_representation": "spherical",
        "input_geometry_units": "Angstrom",
        "pyscf_scf_settings": {
            "conv_tol": case.conv_tol,
            "max_cycle": case.max_cycle,
            "conv_tol_grad": 1e-10 if case.bse_reference else None,
            "init_guess": "1e" if case.bse_reference else "PySCF default",
        },
        "multiplicity": case.spin + 1,
        "input_derived_n_alpha": compare_pyscf.electron_counts(case)[0],
        "input_derived_n_beta": compare_pyscf.electron_counts(case)[1],
        "electron_count_source": "geometry nuclear charges, resolved charge and multiplicity checked for BSE cases",
        "rustiq_scf_iterations": hf.get("iterations"),
        "pyscf_diagnostics": pyscf_diagnostics,
        "rustiq_resolved": rustiq.get("reference_resolved") if rustiq else None,
        "charge": case.charge,
        "spin_n_alpha_minus_n_beta": case.spin,
        "hf_method": hf_method,
        "expected_hf_method": case.method.upper(),
        "ao_basis_dimension": ao_dimension,
        "expected_ao_basis_dimension": case.ao_dimension,
        "rustiq_hf_total_energy_hartree": rhf_energy,
        "pyscf_hf_total_energy_hartree": py_hf,
        "hf_absolute_difference_hartree": error_hf,
        "hf_tolerance_hartree": case.tolerance,
        "rustiq_hf_converged": converged,
        "pyscf_hf_converged": pyscf_hf_converged,
        "hf_pass": hf_ok,
        "status": "FAIL",
        "errors": errors,
    }
    if case.mp2:
        try:
            mp2 = calc["mp2"]
        except (KeyError, TypeError):
            mp2 = None
        py_mp2 = number(pyscf[1]) if pyscf else None
        rust_mp2 = (
            number(mp2.get("correlation_energy")) if isinstance(mp2, dict) else None
        )
        mp2_error = difference(rust_mp2, py_mp2)
        tolerance = case.mp2_tolerance
        expected_mp2_method = f"{case.method.upper()}-MP2"
        mp2_method = mp2.get("method") if isinstance(mp2, dict) else None
        mp2_ok = mp2_method == expected_mp2_method and passed(
            mp2_error, tolerance, rust_mp2, py_mp2
        )
        if mp2_method != expected_mp2_method:
            errors.append("RustiQ MP2 method does not match the reference case")
        if rust_mp2 is None or py_mp2 is None:
            errors.append("MP2 correlation energy is missing or non-finite")
        if not mp2_ok:
            errors.append("MP2 correlation energy comparison failed")
        rust_total = number(mp2.get("total_energy")) if isinstance(mp2, dict) else None
        derived_rust = (
            rhf_energy + rust_mp2
            if rhf_energy is not None and rust_mp2 is not None
            else None
        )
        derived_py = (
            py_hf + py_mp2 if py_hf is not None and py_mp2 is not None else None
        )
        total_error = difference(rust_total, derived_py)
        total_tolerance = case.tolerance + tolerance if tolerance is not None else None
        total_ok = passed(
            difference(rust_total, derived_rust), 1e-12, rust_total, derived_rust
        ) and passed(total_error, total_tolerance, rust_total, derived_py)
        if not total_ok:
            errors.append(
                "MP2 total energy is missing, inconsistent, or outside the summed HF/correlation tolerance"
            )
        result.update(
            {
                "rustiq_mp2_total_energy_hartree": rust_total,
                "pyscf_mp2_total_energy_hartree": derived_py,
                "mp2_total_absolute_difference_hartree": total_error,
                "mp2_total_tolerance_hartree": total_tolerance,
                "mp2_total_pass": total_ok,
                "mp2_reference_frozen_orbitals": case.frozen_orbitals,
                "mp2_frozen_orbitals_convention": "spatial orbitals"
                if case.method == "rhf"
                else "orbitals in each spin channel",
                "rustiq_mp2_method": mp2_method,
                "rustiq_mp2_correlation_energy_hartree": rust_mp2,
                "pyscf_mp2_correlation_energy_hartree": py_mp2,
                "mp2_absolute_difference_hartree": mp2_error,
                "mp2_tolerance_hartree": tolerance,
                "pyscf_mp2_result_available": py_mp2 is not None,
                "mp2_pass": mp2_ok,
            }
        )
    if case.method == "uhf":
        try:
            spin = hf["spin"]
        except (KeyError, TypeError):
            spin = None
        rust_spin = number(spin.get("s_squared")) if isinstance(spin, dict) else None
        py_spin = number(pyscf[2]) if pyscf else None
        spin_error = difference(rust_spin, py_spin)
        spin_ok = passed(spin_error, case.spin_tolerance, rust_spin, py_spin)
        if not spin_ok:
            errors.append("Spin expectation comparison failed or is missing")
        ideal = 0.5 * case.spin * (0.5 * case.spin + 1)
        contamination = (
            number(spin.get("spin_contamination")) if isinstance(spin, dict) else None
        )
        ideal_reported = (
            number(spin.get("ideal_s_squared")) if isinstance(spin, dict) else None
        )
        spin_ok = (
            spin_ok
            and passed(difference(ideal_reported, ideal), 1e-12, ideal_reported)
            and passed(
                difference(
                    contamination, py_spin - ideal if py_spin is not None else None
                ),
                case.spin_tolerance,
                contamination,
            )
        )
        if not spin_ok:
            errors.append("Spin contamination or ideal spin comparison failed")
        result.update(
            {
                "rustiq_s_squared": rust_spin,
                "pyscf_s_squared": py_spin,
                "rustiq_spin_contamination": contamination,
                "pyscf_spin_contamination": py_spin - ideal
                if py_spin is not None
                else None,
                "s_squared_absolute_difference": spin_error,
                "spin_tolerance": case.spin_tolerance,
                "spin_pass": spin_ok,
            }
        )
    elif isinstance(hf, dict) and hf.get("spin") is not None:
        errors.append(
            "RustiQ reported an unexpected open-shell spin diagnostic for RHF"
        )
    if not case.mp2:
        rustiq_mp2 = calc.get("mp2") if isinstance(calc, dict) else None
        pyscf_mp2 = pyscf[1] if pyscf is not None else None
        if rustiq_mp2 is not None or pyscf_mp2 is not None:
            errors.append("An unexpected MP2 result was returned for an HF-only case")
    result["status"] = (
        "PASS"
        if (
            not errors
            and hf_ok
            and (not case.mp2 or result.get("mp2_pass") is True)
            and (case.method != "uhf" or result.get("spin_pass") is True)
        )
        else "FAIL"
    )
    return result


def metadata() -> dict[str, Any]:
    def command(*args: str) -> str | None:
        try:
            return subprocess.run(
                args, cwd=REPO_ROOT, text=True, capture_output=True, check=True
            ).stdout.strip()
        except (OSError, subprocess.CalledProcessError):
            return None

    try:
        from pyscf import __version__ as pyscf_version
    except ImportError:
        pyscf_version = None
    cargo = tomllib.loads((REPO_ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    return {
        "rustiq_version": cargo["package"]["version"],
        "git_commit": command("git", "rev-parse", "HEAD"),
        "working_tree_clean": command("git", "status", "--porcelain") == "",
        "pyscf_version": pyscf_version,
        "python_version": platform.python_version(),
        "rustc_version": command("rustc", "--version"),
        "operating_system": platform.platform(),
        "cpu_architecture": platform.machine(),
        "generated_at_utc": datetime.now(UTC).isoformat(),
    }


def molecular_formula(path: Path) -> str:
    counts: dict[str, int] = {}
    for line in path.read_text(encoding="utf-8").splitlines()[2:]:
        fields = line.split()
        if fields:
            symbol = fields[0]
            if symbol.isdecimal():
                # The shared H2 geometry uses XYZ's atomic-number spelling for hydrogen.
                symbol = {"1": "H"}.get(symbol, symbol)
            counts[symbol] = counts.get(symbol, 0) + 1
    if counts == {"Li": 1, "H": 1}:
        return "LiH"
    if counts == {"H": 1, "O": 1}:
        return "OH"
    return "".join(
        symbol + (str(counts[symbol]) if counts[symbol] > 1 else "")
        for symbol in sorted(counts)
    )


def render_markdown(report: dict[str, Any]) -> str:
    rows = report["cases"]
    fmt = lambda n: "—" if n is None else f"{n:.17e}"
    lines = [
        "# RustiQ Scientific Validation",
        "",
        "## Experimental status and limitations",
        "",
        "Agreement with PySCF validates only the selected numerical cases; it does not establish general chemical accuracy. The reference suite is not exhaustive. HF omits electron correlation, and MP2 can be unreliable for strong correlation or small energy denominators. Matching convergence thresholds do not imply identical convergence criteria.",
        "",
        "## Reproducibility information",
        "",
        "```json",
        json.dumps(report["metadata"], indent=2, sort_keys=True),
        "```",
        "",
        "## Reference cases",
        "",
        "Spin is Nα−Nβ=2M_S. The AO dimension and method shown are read from RustiQ JSON and checked against the case definition.",
        "",
        "| Case | Molecule | Geometry | Basis | Charge | Spin (Nα−Nβ) | AO dimension | Method |",
        "|---|---|---|---|---:|---:|---:|---|",
    ]
    for r in rows:
        lines.append(
            f"| {r['case_id']} | {r['molecule']} | `{r['geometry_path']}` | {r['basis']} | {r['charge']} | {r['spin_n_alpha_minus_n_beta']} | {r['ao_basis_dimension'] if r['ao_basis_dimension'] is not None else '—'} | {r['hf_method'] or '—'} |"
        )
    lines += [
        "",
        "## Hartree-Fock validation",
        "",
        "Energies below are total molecular HF energies in Hartree, including nuclear repulsion.",
        "",
        "| Case | Method | RustiQ total (Ha) | PySCF total (Ha) | Absolute error (Ha) | Tolerance (Ha) | Status |",
        "|---|---:|---:|---:|---:|---:|---|",
    ]
    for r in rows:
        lines.append(
            f"| {r['case_id']} | {r['hf_method']} | {fmt(r['rustiq_hf_total_energy_hartree'])} | {fmt(r['pyscf_hf_total_energy_hartree'])} | {fmt(r['hf_absolute_difference_hartree'])} | {fmt(r['hf_tolerance_hartree'])} | {'PASS' if r['hf_pass'] else 'FAIL'} |"
        )
    lines += [
        "",
        "## MP2 validation",
        "",
        "MP2 values are correlation energies only; they exclude the HF energy and nuclear repulsion.",
        "",
        "| Case | HF method | Frozen orbitals | RustiQ correlation (Ha) | PySCF correlation (Ha) | Absolute error (Ha) | Tolerance (Ha) | Status |",
        "|---|---|---:|---:|---:|---:|---:|---|",
    ]
    for r in rows:
        if "mp2_pass" in r:
            lines.append(
                f"| {r['case_id']} | {r['hf_method'] or '—'} | {r.get('mp2_reference_frozen_orbitals', 0)} | {fmt(r['rustiq_mp2_correlation_energy_hartree'])} | {fmt(r['pyscf_mp2_correlation_energy_hartree'])} | {fmt(r['mp2_absolute_difference_hartree'])} | {fmt(r['mp2_tolerance_hartree'])} | {'PASS' if r['mp2_pass'] else 'FAIL'} |"
            )
    lines += [
        "",
        "## Open-shell spin diagnostics",
        "",
        "| Case | RustiQ <S²> | PySCF <S²> | Absolute difference | Tolerance | RustiQ contamination | PySCF contamination | Status |",
        "|---|---:|---:|---:|---:|---:|---:|---|",
    ]
    for r in rows:
        if "spin_pass" in r:
            lines.append(
                f"| {r['case_id']} | {fmt(r['rustiq_s_squared'])} | {fmt(r['pyscf_s_squared'])} | {fmt(r['s_squared_absolute_difference'])} | {fmt(r['spin_tolerance'])} | {fmt(r['rustiq_spin_contamination'])} | {fmt(r['pyscf_spin_contamination'])} | {'PASS' if r['spin_pass'] else 'FAIL'} |"
            )
    lines += [
        "",
        "## Geometry provenance and convergence",
        "",
        "Provenance and reuse conditions are recorded in DATA_SOURCES.md. Coordinates and basis hashes, resolved RustiQ snapshots, electron counts, virtual counts, and PySCF SCF cycles are retained in validation.json.",
        "",
        "| Case | Geometry provenance | RustiQ cycles | PySCF cycles |",
        "|---|---|---:|---:|",
    ]
    for r in rows:
        diagnostics = r.get("pyscf_diagnostics") or {}
        lines.append(
            f"| {r['case_id']} | {r.get('geometry_provenance', '—')} | {r.get('rustiq_scf_iterations', '—')} | {diagnostics.get('iterations', '—')} |"
        )
    lines += [
        "",
        "## Derived MP2 total energies",
        "",
        "Totals are HF total plus MP2 correlation, in Hartree. Acceptance uses the sum of the separate absolute tolerances and checks RustiQ's reported total against its own sum within 1e-12 Ha.",
        "",
        "| Case | RustiQ total (Ha) | PySCF total (Ha) | Absolute error (Ha) | Tolerance (Ha) | Status |",
        "|---|---:|---:|---:|---:|---|",
    ]
    for r in rows:
        if "mp2_total_pass" in r:
            lines.append(
                f"| {r['case_id']} | {fmt(r['rustiq_mp2_total_energy_hartree'])} | {fmt(r['pyscf_mp2_total_energy_hartree'])} | {fmt(r['mp2_total_absolute_difference_hartree'])} | {fmt(r['mp2_total_tolerance_hartree'])} | {'PASS' if r['mp2_total_pass'] else 'FAIL'} |"
            )
    integral_report = report.get("integrals")
    if integral_report:
        lines += [
            "",
            "## Independent AO integral validation",
            "",
            "Complete one-electron matrices and unscreened compact ERIs; atomic units. Internal eightfold reconstruction is checked separately by Cargo.",
            "",
            "| Case | AO representation | AOs | Family | Values | Max absolute error | RMS error | Absolute tolerance | Status |",
            "|---|---|---:|---|---:|---:|---:|---:|---|",
        ]
        for r in integral_report.get("cases", []):
            lines.append(
                f"| {r['case_id']} | {r['representation']} | {r['nao']} | {r['family']} | {r['count']} | {fmt(r['max_absolute_error'])} | {fmt(r['rms_absolute_error'])} | {fmt(r['absolute_tolerance'])} | {r['status']} |"
            )
        if integral_report.get("error") or integral_report.get("cargo_error"):
            lines.append(
                f"Integral validation failed: {integral_report.get('error') or integral_report['cargo_error']}"
            )
    failed = [r for r in rows if r["status"] != "PASS"]
    lines += [
        "",
        "## Summary",
        "",
        f"{len(rows) - len(failed)}/{len(rows)} cases passed; {len(failed)} failed.",
    ]
    if integral_report:
        lines.append(
            f"Independent AO integrals: {'PASS' if integral_report['passed'] else 'FAIL'}."
        )
    if failed:
        lines += ["", "Failures:"]
        for r in failed:
            lines.append(
                f"- **{r['case_id']}**: {'; '.join(r['errors']) or 'validation failed'}"
            )
    lines += [
        "",
        "A frozen count applies to spatial orbitals for RHF and to orbitals in each spin channel for UHF; the exact convention is included in the JSON.",
        "",
        "## Scientific limitations",
        "",
        "This suite checks numerical agreement for the listed geometries, basis sets, charge and spin states, and methods. It does not assess basis-set completeness, broad chemical accuracy, or the suitability of HF/MP2 for arbitrary systems. The reported <S²> is the HF reference spin diagnostic.",
        "",
    ]
    return "\n".join(lines)


def generate(output_dir: Path) -> dict[str, Any]:
    env = compare_pyscf.prepare_rustiq_env()
    cases = list(compare_pyscf.CASES)
    work = output_dir / ".inputs"
    for original in (
        case for case in compare_pyscf.CASES if case.frozen_core_reference
    ):
        for budget in compare_pyscf.FROZEN_CORE_MEMORY_BUDGETS:
            variant = frozen_runfile(original, budget, work)
            cases.append(
                replace(
                    variant, name=f"{original.name}-frozen1-{budget.replace(' ', '-')}"
                )
            )
    results = []
    try:
        for case in cases:
            errors: list[str] = []
            rustiq = pyscf = None
            pyscf_hf_converged = None
            diagnostics: dict = {}
            try:
                rustiq = compare_pyscf.rustiq_result(case, env)
            except Exception as error:  # noqa: BLE001 - preserve every calculation failure in the report
                errors.append(f"RustiQ calculation failed: {error}")
            try:
                pyscf = compare_pyscf.pyscf_result(case, diagnostics)
                pyscf_hf_converged = True
            except compare_pyscf.PyscfMp2Error as error:
                pyscf = error.hf_result
                pyscf_hf_converged = True
                errors.append(str(error))
            except compare_pyscf.PyscfScfConvergenceError as error:
                pyscf_hf_converged = False
                errors.append(f"PySCF HF did not converge: {error}")
            except Exception as error:  # noqa: BLE001 - preserve every calculation failure in the report
                errors.append(f"PySCF calculation failed: {error}")
            results.append(
                validate_case(
                    case, rustiq, pyscf, errors, pyscf_hf_converged, diagnostics
                )
            )
    finally:
        shutil.rmtree(Path(env["RUSTIQ_DATA_HOME"]), ignore_errors=True)
        shutil.rmtree(work, ignore_errors=True)
    report = {
        "schema_version": 1,
        "metadata": metadata(),
        "tolerances": {
            c.name: {
                "hf_hartree": c.tolerance,
                "mp2_hartree": c.mp2_tolerance,
                "spin_s_squared": c.spin_tolerance if c.method == "uhf" else None,
            }
            for c in cases
        },
        "reference_case_ids": [c.name for c in cases],
        "cases": results,
        "passed": all(r["status"] == "PASS" for r in results),
    }
    try:
        import integrals

        report["integrals"] = integrals.validate()
    except Exception as error:  # noqa: BLE001 - preserve harness failures in the report
        report["integrals"] = {"passed": False, "error": str(error), "cases": []}
    report["passed"] = report["passed"] and report["integrals"]["passed"]
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "validation.json").write_text(
        json.dumps(report, indent=2, sort_keys=True, allow_nan=False) + "\n",
        encoding="utf-8",
    )
    (output_dir / "validation.md").write_text(render_markdown(report), encoding="utf-8")
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output-dir", type=Path, default=Path("target/scientific-report")
    )
    args = parser.parse_args()
    report = generate(args.output_dir)
    print(
        f"Scientific validation: {sum(r['status'] == 'PASS' for r in report['cases'])}/{len(report['cases'])} energy cases passed; AO integrals {'PASS' if report['integrals']['passed'] else 'FAIL'}"
    )
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
