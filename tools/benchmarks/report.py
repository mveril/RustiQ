#!/usr/bin/env python3
"""Summarize structured Criterion results and metadata emitted by RustiQ benches."""
from __future__ import annotations

import argparse
import json
import math
import re
import subprocess
import tomllib
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]


class ReportError(ValueError):
    """Criterion data is absent or cannot support the requested report."""


def read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ReportError(f"Invalid or missing JSON: {path}: {error}") from error


def finite(value: Any, label: str) -> float:
    try:
        result = float(value)
    except (TypeError, ValueError) as error:
        raise ReportError(f"{label} must be numeric") from error
    if not math.isfinite(result):
        raise ReportError(f"{label} must be finite")
    return result


def criterion_measurement(
    root: Path, benchmark_id: str, not_before: float
) -> dict[str, Any]:
    # Criterion 0.8 emits structured benchmark and estimate JSON under new/.
    encoded = benchmark_id.replace("/", "_")
    for path in root.rglob("new/benchmark.json"):
        data = read_json(path)
        if not isinstance(data, dict):
            raise ReportError(f"Criterion benchmark metadata must be a JSON object: {path}")
        if data.get("full_id") == benchmark_id or path.parent.parent.name == encoded:
            estimates_path = path.parent / "estimates.json"
            try:
                if estimates_path.stat().st_mtime < not_before:
                    raise ReportError(
                        f"Criterion results for {benchmark_id} predate this benchmark run"
                    )
            except OSError as error:
                raise ReportError(f"Missing Criterion estimates: {estimates_path}") from error
            estimates = read_json(estimates_path)
            if not isinstance(estimates, dict):
                raise ReportError(f"Criterion estimates must be a JSON object: {path.parent / 'estimates.json'}")
            mean = estimates.get("mean", {})
            if not isinstance(mean, dict):
                raise ReportError(f"Criterion mean estimate is invalid for {benchmark_id}")
            interval = mean.get("confidence_interval", {})
            if not isinstance(interval, dict):
                raise ReportError(f"Criterion confidence interval is invalid for {benchmark_id}")
            estimate = finite(mean.get("point_estimate"), f"{benchmark_id} mean")
            lower = finite(interval.get("lower_bound"), f"{benchmark_id} CI lower")
            upper = finite(interval.get("upper_bound"), f"{benchmark_id} CI upper")
            confidence = finite(interval.get("confidence_level"), f"{benchmark_id} CI confidence")
            if estimate <= 0 or lower < 0 or lower > upper or not lower <= estimate <= upper or not 0 < confidence < 1:
                raise ReportError(f"Invalid confidence interval for {benchmark_id}")
            return {"benchmark_id": benchmark_id, "mean_seconds": estimate / 1e9,
                    "confidence_interval_seconds": [lower / 1e9, upper / 1e9],
                    "confidence_level": confidence, "throughput": data.get("throughput")}
    raise ReportError(f"Missing Criterion results for {benchmark_id} under {root}")


def machine_metadata(eri_run: dict[str, Any], mp2_run: dict[str, Any]) -> dict[str, Any]:
    fields = ("git_commit", "working_tree_clean", "rustc_version", "operating_system", "architecture", "cpu_model", "benchmark_profile")
    for key in fields:
        if eri_run.get(key) != mp2_run.get(key):
            raise ReportError(f"ERI and MP2 benchmark provenance differs for {key}")
    if not eri_run.get("git_commit") or not eri_run.get("rustc_version"):
        raise ReportError("Benchmark execution did not record a Git commit and Rust compiler version")
    try:
        current_commit = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True,
                                        capture_output=True, check=True).stdout.strip()
        current_status = subprocess.run(["git", "status", "--porcelain"], cwd=ROOT, text=True,
                                        capture_output=True, check=True).stdout
    except (OSError, subprocess.CalledProcessError) as error:
        raise ReportError(f"Could not verify benchmark source revision: {error}") from error
    if eri_run["git_commit"] != current_commit:
        raise ReportError("Criterion measurements were made at a different Git commit")
    if eri_run.get("working_tree_clean") != (current_status == ""):
        raise ReportError("Benchmark working-tree state differs from the report-generation source tree")
    try:
        started = min(finite(eri_run["benchmark_started_unix_seconds"], "ERI timestamp"),
                      finite(mp2_run["benchmark_started_unix_seconds"], "MP2 timestamp"))
    except KeyError as error:
        raise ReportError(f"Missing benchmark execution timestamp: {error}") from error
    try:
        cargo = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
        version = cargo["package"]["version"]
    except (OSError, KeyError, tomllib.TOMLDecodeError):
        version = None
    return {"rustiq_version": version, **{key: eri_run[key] for key in fields},
            "benchmark_started_at_utc": datetime.fromtimestamp(started, UTC).isoformat(),
            "eri_benchmark_started_at_utc": datetime.fromtimestamp(finite(eri_run["benchmark_started_unix_seconds"], "ERI timestamp"), UTC).isoformat(),
            "mp2_benchmark_started_at_utc": datetime.fromtimestamp(finite(mp2_run["benchmark_started_unix_seconds"], "MP2 timestamp"), UTC).isoformat(),
            "report_generated_at_utc": datetime.now(UTC).isoformat(),
            "criterion_version": _criterion_version(), "results_machine_dependent": True}


def _criterion_version() -> str | None:
    try:
        lock = (ROOT / "Cargo.lock").read_text(encoding="utf-8")
    except OSError:
        return None
    match = re.search(r'(?ms)^\[\[package\]\]\nname = "criterion"\nversion = "([^\"]+)"', lock)
    return match.group(1) if match else None


def generate(criterion_dir: Path, output_dir: Path) -> dict[str, Any]:
    eri_file = read_json(criterion_dir / "eri-metadata.json")
    mp2_file = read_json(criterion_dir / "mp2-metadata.json")
    if not isinstance(eri_file, dict) or not isinstance(mp2_file, dict):
        raise ReportError("ERI and MP2 metadata must be JSON objects")
    eri_meta, mp2_meta = eri_file.get("cases"), mp2_file.get("cases")
    eri_run, mp2_run = eri_file.get("run"), mp2_file.get("run")
    if not isinstance(eri_meta, list) or not eri_meta or not isinstance(mp2_meta, list) or not mp2_meta:
        raise ReportError("ERI and MP2 benchmark metadata must both contain executed cases")
    if not isinstance(eri_run, dict) or not isinstance(mp2_run, dict):
        raise ReportError("Missing benchmark execution provenance")
    eri = []
    for item in eri_meta:
        if not isinstance(item, dict) or not isinstance(item.get("benchmark_id"), str):
            raise ReportError("Invalid ERI benchmark metadata entry")
        measurement = criterion_measurement(
            criterion_dir,
            item["benchmark_id"],
            finite(eri_run.get("benchmark_started_unix_seconds"), "ERI timestamp"),
        )
        throughput = measurement.pop("throughput")
        eps = None
        if isinstance(throughput, dict) and len(throughput) == 1:
            eps = finite(next(iter(throughput.values())), "ERI throughput") / measurement["mean_seconds"]
        eri.append({**item, **measurement, "throughput_elements_per_second": eps})
    mp2 = []
    for item in mp2_meta:
        if not isinstance(item, dict) or not isinstance(item.get("ao_dimension"), int):
            raise ReportError("Invalid MP2 benchmark metadata entry")
        n = item["ao_dimension"]
        mp2_started = finite(mp2_run.get("benchmark_started_unix_seconds"), "MP2 timestamp")
        dense = criterion_measurement(criterion_dir, f"mp2/dense/{n}", mp2_started)
        blocked = criterion_measurement(criterion_dir, f"mp2/blocked/{n}", mp2_started)
        if finite(item.get("energy_absolute_difference"), f"MP2 N={n} consistency") >= 1e-12:
            raise ReportError(f"Dense and blocked MP2 energy consistency check failed for N={n}")
        if finite(item.get("workspace_budget_bytes"), "workspace budget") <= 0 or item.get("block_size", 0) <= 0:
            raise ReportError(f"Invalid MP2 configuration metadata for N={n}")
        mp2.append({**item, "dense": dense, "blocked": blocked,
                    "dense_over_blocked_ratio": dense["mean_seconds"] / blocked["mean_seconds"]})
    report = {"schema_version": 1, "metadata": machine_metadata(eri_run, mp2_run),
              "criterion_configuration": {
                  "eri": [{key: item[key] for key in ("benchmark_id", "criterion_sample_size", "criterion_warm_up_seconds", "criterion_measurement_seconds")} for item in eri_meta],
                  "mp2": [{key: item[key] for key in ("ao_dimension", "criterion_sample_size", "criterion_warm_up_seconds", "criterion_measurement_seconds")} for item in mp2_meta]},
              "rayon_threads": sorted({item["rayon_threads"] for item in mp2}),
              "eri": eri, "mp2": mp2,
              "memory_note": "Configured MP2 workspace budget is not measured peak process memory. Peak RSS was not independently measured.",
              "mp2_scope_note": "Synthetic MP2 cases time AO-to-MO transformation and correlation-energy evaluation; they do not represent complete molecular calculations."}
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "benchmarks.json").write_text(json.dumps(report, indent=2, sort_keys=True, allow_nan=False) + "\n", encoding="utf-8")
    (output_dir / "benchmarks.md").write_text(render_markdown(report), encoding="utf-8")
    return report


def render_markdown(report: dict[str, Any]) -> str:
    fmt = lambda value: f"{value:.6g}"
    lines = ["# RustiQ Performance Benchmarks", "", "Results are machine-dependent and come from Criterion measurements. No comparison against other quantum chemistry packages is made.", "", "## Reproducibility information", "", "```json", json.dumps(report["metadata"], indent=2, sort_keys=True), "```", "", "Sample sizes, warm-up and measurement targets are recorded per case in `benchmarks.json`.", "", "## ERI computation", "", "| Case | Basis | AO functions | Compact integrals | Mean (s) | Criterion CI (s) | Compact elements/s |", "|---|---|---:|---:|---:|---:|---:|"]
    for row in report["eri"]:
        lo, hi = row["confidence_interval_seconds"]
        level = round(row["confidence_level"] * 100)
        throughput = fmt(row["throughput_elements_per_second"]) if row["throughput_elements_per_second"] is not None else "—"
        lines.append(f"| {row['case']} | {row['basis']} | {row['basis_functions']} | {row['compact_integrals']} | {fmt(row['mean_seconds'])} | {level}% [{fmt(lo)}, {fmt(hi)}] | {throughput} |")
    lines += ["", "Throughput counts compact storage elements per second; Schwarz screening may skip evaluations or leave stored values zero.", "", "## MP2 AO-to-MO transformation and correlation-energy evaluation", "", "Synthetic AO-dimension cases time the transformation and correlation-energy evaluation, not full molecular calculations.", "", "| AO dim | Occupied | Dense mean (s) | Dense CI (s) | Blocked mean (s) | Blocked CI (s) | Dense/blocked | Workspace budget | Block size | Rayon threads |", "|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|"]
    for row in report["mp2"]:
        dlo, dhi = row["dense"]["confidence_interval_seconds"]
        blo, bhi = row["blocked"]["confidence_interval_seconds"]
        dlevel = round(row["dense"]["confidence_level"] * 100)
        blevel = round(row["blocked"]["confidence_level"] * 100)
        lines.append(f"| {row['ao_dimension']} | {row['occupied_orbitals']} | {fmt(row['dense']['mean_seconds'])} | {dlevel}% [{fmt(dlo)}, {fmt(dhi)}] | {fmt(row['blocked']['mean_seconds'])} | {blevel}% [{fmt(blo)}, {fmt(bhi)}] | {fmt(row['dense_over_blocked_ratio'])} | {row['workspace_budget_bytes']} B | {row['block_size']} | {row['rayon_threads']} |")
    lines += ["", "## Memory interpretation", "", report["memory_note"], "", "Configured workspace is a per-calculation budget for the blocked MP2 transformation; it is not measured peak process memory.", ""]
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--criterion-dir", type=Path, default=Path("target/criterion"))
    parser.add_argument("--output-dir", type=Path, default=Path("target/performance-report"))
    args = parser.parse_args()
    try:
        report = generate(args.criterion_dir, args.output_dir)
    except ReportError as error:
        parser.error(str(error))
    print(f"Performance report written for {len(report['eri'])} ERI and {len(report['mp2'])} MP2 cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
