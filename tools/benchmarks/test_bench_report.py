from __future__ import annotations

import importlib.util
import json
from pathlib import Path

import pytest

SPEC = importlib.util.spec_from_file_location("performance_report", Path(__file__).with_name("report.py"))
assert SPEC and SPEC.loader
report = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(report)


def write_criterion(root: Path, benchmark_id: str, mean: float = 2e6) -> None:
    directory = root / "group" / benchmark_id.replace("/", "_") / "new"
    directory.mkdir(parents=True)
    (directory / "benchmark.json").write_text(json.dumps({"full_id": benchmark_id,
        "throughput": {"Elements": 16}}))
    (directory / "estimates.json").write_text(json.dumps({"mean": {
        "point_estimate": mean, "confidence_interval": {"confidence_level": 0.95,
        "lower_bound": mean * 0.9, "upper_bound": mean * 1.1}}}))


def test_reads_criterion_estimates_and_confidence_interval(tmp_path: Path) -> None:
    write_criterion(tmp_path, "eri/h2-sto3g/sto-3g")
    result = report.criterion_measurement(tmp_path, "eri/h2-sto3g/sto-3g", 0)
    assert result["mean_seconds"] == 0.002
    assert result["confidence_interval_seconds"] == [0.0018, 0.0022]


def test_invalid_and_missing_criterion_results_fail(tmp_path: Path) -> None:
    with pytest.raises(report.ReportError, match="Missing Criterion"):
        report.criterion_measurement(tmp_path, "mp2/dense/12", 0)
    write_criterion(tmp_path, "mp2/dense/12", float("nan"))
    with pytest.raises(report.ReportError, match="finite"):
        report.criterion_measurement(tmp_path, "mp2/dense/12", 0)


def test_stale_criterion_results_are_rejected(tmp_path: Path) -> None:
    write_criterion(tmp_path, "eri/h2-sto3g/sto-3g")
    with pytest.raises(report.ReportError, match="predate this benchmark run"):
        report.criterion_measurement(tmp_path, "eri/h2-sto3g/sto-3g", 4e9)


def test_invalid_json_is_diagnostic(tmp_path: Path) -> None:
    path = tmp_path / "invalid.json"
    path.write_text("not json")
    with pytest.raises(report.ReportError, match="Invalid or missing JSON"):
        report.read_json(path)


def test_performance_markdown_separates_budget_from_peak_memory() -> None:
    row = {"case": "h2", "basis": "sto-3g", "basis_functions": 2,
        "compact_integrals": 6, "mean_seconds": 0.01,
        "confidence_interval_seconds": [0.009, 0.011],
        "confidence_level": 0.95,
        "throughput_elements_per_second": 600.0}
    mp = {"ao_dimension": 12, "occupied_orbitals": 4, "dense": {"mean_seconds": 0.02,
        "confidence_interval_seconds": [0.019, 0.021], "confidence_level": 0.95}, "blocked": {"mean_seconds": 0.01,
        "confidence_interval_seconds": [0.009, 0.011], "confidence_level": 0.95}, "dense_over_blocked_ratio": 2.0,
        "workspace_budget_bytes": 32768, "block_size": 2, "block_count": 2, "rayon_threads": 4}
    md = report.render_markdown({"metadata": {}, "eri": [row], "mp2": [mp],
        "memory_note": "Configured budget; peak RSS was not measured."})
    assert "Dense/multiblock" in md
    assert "peak RSS was not measured" in md
    assert "Compact elements/s" in md


def test_execution_provenance_must_match() -> None:
    run = {"git_commit": "abc", "working_tree_clean": True, "rustc_version": "rustc test", "operating_system": "linux",
        "architecture": "x86_64", "cpu_model": "cpu", "benchmark_profile": "bench",
        "benchmark_started_unix_seconds": 1.0}
    other = {**run, "git_commit": "different"}
    with pytest.raises(report.ReportError, match="provenance differs"):
        report.machine_metadata(run, other)
