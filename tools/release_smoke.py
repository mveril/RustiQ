#!/usr/bin/env python3
"""Validate a cargo-dist RustiQ archive by running its packaged executable."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import zipfile
from pathlib import Path


def verify_checksum(archive: Path, checksum_file: Path) -> None:
    text = checksum_file.read_text(encoding="utf-8").strip()
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    matching_lines = [line for line in text.splitlines() if archive.name in line]
    if matching_lines:
        text = "\n".join(matching_lines)
    candidates = re.findall(r"(?i)\b([0-9a-f]{64})\b", text)
    if digest not in [value.lower() for value in candidates]:
        raise ValueError(f"SHA-256 mismatch for {archive.name} using {checksum_file.name}")


def safe_extract(archive: Path, destination: Path) -> None:
    if zipfile.is_zipfile(archive):
        with zipfile.ZipFile(archive) as bundle:
            for member in bundle.infolist():
                target = (destination / member.filename).resolve()
                if not target.is_relative_to(destination.resolve()):
                    raise ValueError(f"unsafe archive path: {member.filename}")
            bundle.extractall(destination)
        return
    if tarfile.is_tarfile(archive):
        with tarfile.open(archive, "r:*") as bundle:
            for member in bundle.getmembers():
                target = (destination / member.name).resolve()
                if not target.is_relative_to(destination.resolve()) or member.issym() or member.islnk():
                    raise ValueError(f"unsafe archive member: {member.name}")
            bundle.extractall(destination, filter="data")
        return
    raise ValueError(f"unsupported or invalid archive: {archive}")


def run(binary: Path, args: list[str], cwd: Path, env: dict[str, str], log: Path) -> str:
    result = subprocess.run([str(binary), *args], cwd=cwd, env=env, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    log.write_text(f"$ {binary.name} {' '.join(args)}\n--- stdout ---\n{result.stdout}\n--- stderr ---\n{result.stderr}\nexit={result.returncode}\n", encoding="utf-8")
    if result.returncode:
        raise RuntimeError(f"{binary.name} {' '.join(args)} failed ({result.returncode}); see {log.name}: {result.stderr.strip()}")
    return result.stdout


def validate(archive: Path, fixture: Path, geometry: Path, reference_source: Path, log_dir: Path,
             checksum: Path | None = None, expected_energy: float | None = None,
             command_runner=run) -> dict[str, object]:
    archive = archive.resolve()
    fixture = fixture.resolve()
    geometry = geometry.resolve()
    log_dir.mkdir(parents=True, exist_ok=True)
    if checksum:
        verify_checksum(archive, checksum.resolve())
    elif Path(str(archive) + ".sha256").exists():
        checksum = Path(str(archive) + ".sha256")
        verify_checksum(archive, checksum)
    elif (archive.parent / "sha256.sum").exists():
        checksum = archive.parent / "sha256.sum"
        lines = [line for line in checksum.read_text(encoding="utf-8").splitlines() if archive.name in line]
        if len(lines) != 1:
            raise ValueError(f"no unique dist checksum entry for {archive.name} in sha256.sum")
        verify_checksum(archive, checksum)
    else:
        raise ValueError("dist archive checksum file was not found")

    match = re.search(r"PYSCF_TOTAL_ENERGY:\s*f64\s*=\s*(-?[0-9._]+)", reference_source.read_text(encoding="utf-8"))
    if expected_energy is None:
        if not match:
            raise ValueError("could not read H2/STO-3G reference from regression source")
        expected_energy = float(match.group(1).replace("_", ""))

    with tempfile.TemporaryDirectory(prefix="rustiq-release-smoke-") as temporary:
        root = Path(temporary)
        extracted = root / "extracted"
        work = root / "work"
        basis = root / "basis"
        extracted.mkdir()
        work.mkdir()
        basis.mkdir()
        safe_extract(archive, extracted)
        binaries = [p for p in extracted.rglob("rustiq*") if p.is_file() and p.name.lower() in {"rustiq", "rustiq.exe"}]
        if len(binaries) != 1:
            raise ValueError(f"expected one rustiq executable in archive, found {len(binaries)}")
        binary = binaries[0]
        if os.name != "nt":
            binary.chmod(binary.stat().st_mode | 0o111)
        clean_path = os.pathsep.join([str(Path(os.environ.get("SystemRoot", "C:/Windows")) / "System32") if os.name == "nt" else "/usr/bin", "/bin"])
        env = {"PATH": clean_path, "HOME": str(root), "USERPROFILE": str(root), "APPDATA": str(root / "appdata"), "XDG_CACHE_HOME": str(root / "cache"), "RUSTIQ_DATA_BASIS": str(basis), "RUSTIQ_CACHE_HOME": str(root / "cache"), "RUSTIQ_AUTO_DOWNLOAD": "0", "RUSTIQ_COLOR": "never", "NO_COLOR": "1"}
        if os.name == "nt":
            env["SystemRoot"] = os.environ.get("SystemRoot", "C:\\Windows")
        help_output = command_runner(binary, ["--help"], work, env, log_dir / "help.log")
        if not help_output.strip():
            raise ValueError("rustiq --help produced no output")
        command_runner(binary, ["basis", "import", str(fixture)], work, env, log_dir / "basis-import.log")
        shutil.copyfile(geometry, work / "h2.xyz")
        command_runner(binary, ["init", "h2.xyz", "--output", "calculation.toml", "--hf", "rhf"], work, env, log_dir / "init.log")
        output = command_runner(binary, ["run", "calculation.toml", "--format", "json", "--no-auto-download", "--color", "never"], work, env, log_dir / "calculation.log")
        if "\x1b[" in output:
            raise ValueError("ANSI escape sequence found in JSON output")
        result = json.loads(output)
        hf = result["calculation"]["hf"]
        if result.get("schema_version") != 1:
            raise ValueError(f"unexpected schema version: {result.get('schema_version')}")
        if hf.get("converged") is not True:
            raise ValueError("HF calculation did not converge")
        if hf.get("method") != "RHF":
            raise ValueError(f"expected RHF, got {hf.get('method')}")
        energy = float(hf["total_energy"])
        if abs(energy - expected_energy) > 1e-5:
            raise ValueError(f"HF energy {energy:.12f} differs from regression reference {expected_energy:.12f} by more than 1e-5 Eh")
        return {"checksum": "verified", "startup": "passed", "h2": f"RHF converged; {energy:.12f} Eh"}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--checksum", type=Path)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--geometry", type=Path, required=True)
    parser.add_argument("--reference-source", type=Path, required=True)
    parser.add_argument("--logs", type=Path, default=Path("release-smoke-logs"))
    args = parser.parse_args()
    try:
        status = validate(args.archive, args.fixture, args.geometry, args.reference_source, args.logs, args.checksum)
        print(f"PASS: {args.archive.name}: {status['h2']}; SHA-256 verified")
        return 0
    except Exception as error:  # diagnostic boundary for CI
        print(f"FAIL: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
