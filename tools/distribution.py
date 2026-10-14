#!/usr/bin/env python3
"""Prepare license notices and corresponding source for RustiQ distributions."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import tarfile
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
NOTICES = ROOT / "THIRD-PARTY-NOTICES.txt"


def run(*args: str, cwd: Path = ROOT) -> str:
    return subprocess.check_output(args, cwd=cwd, text=True)


def lock_digest() -> str:
    return hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest()


def notice_inputs_digest() -> str:
    digest = hashlib.sha256()
    paths = [
        "Cargo.lock",
        "Cargo.toml",
        "crates/rustiq-core/Cargo.toml",
        "about.toml",
        "about.hbs",
        "tools/distribution.py",
        "licenses/extra-notices.txt",
    ]
    paths.extend(
        path.relative_to(ROOT).as_posix()
        for path in sorted((ROOT / "third-party").rglob("*"))
        if path.is_file()
    )
    for name in paths:
        digest.update(name.encode())
        digest.update((ROOT / name).read_bytes())
    return digest.hexdigest()


def metadata(offline: bool) -> dict:
    args = ["cargo", "metadata", "--locked", "--all-features", "--format-version", "1"]
    if offline:
        args.append("--offline")
    return json.loads(run(*args))


def prepare_notices(offline: bool, cargo_about: str) -> None:
    args = [
        cargo_about,
        "generate",
        "--locked",
        "--workspace",
        "--all-features",
        "--fail",
        "about.hbs",
    ]
    if offline:
        args.append("--offline")
    text = run(*args)
    supplement = ROOT / "licenses" / "extra-notices.txt"
    if supplement.exists():
        text += "\n" + supplement.read_text(encoding="utf-8")
    packages = sorted(
        metadata(offline)["packages"], key=lambda p: (p["name"], p["version"])
    )
    version = next(p["version"] for p in packages if p["name"] == "rustiq-chem")
    source_url = f"https://github.com/mveril/RustiQ/releases/download/v{version}/rustiq-corresponding-source.tar.gz"
    header = f"Cargo.lock SHA-256: {lock_digest()}\nNotice inputs SHA-256: {notice_inputs_digest()}\nCorresponding source: {source_url}\n\n"
    additions = [
        "\nAdditional upstream notices and copyright statements\n====================================================\n"
    ]
    # cargo-about selects license alternatives. Preserve separate Apache NOTICE
    # files and licenses of bundled native code/assets as well as crate licenses.
    seen_files: dict[str, str] = {}
    for package in packages:
        if package["name"] in {"rustiq-chem", "rustiq-chem-core"}:
            continue
        root = Path(package["manifest_path"]).parent
        additions.append(f"\nPACKAGE: {package['name']} {package['version']}\n")
        additions.append(f"Declared license: {package['license']}\n")
        additions.append(f"Repository: {package['repository'] or '(not declared)'}\n")
        additions.append(
            f"Authors: {', '.join(package['authors']) or '(not declared)'}\n"
        )
        copyrights = set()
        for path in sorted(root.rglob("*")):
            if not path.is_file():
                continue
            name = path.name.lower()
            if re.match(
                r"^(licen[cs]e|copying|copyright|notice|unlicense)(?:$|[._-])", name
            ):
                content = path.read_text(encoding="utf-8", errors="replace")
                label = f"{package['name']} {package['version']} / {path.relative_to(root).as_posix()}"
                digest = hashlib.sha256(content.encode()).hexdigest()
                reference = seen_files.get(digest)
                additions.append(f"\nFILE: {path.relative_to(root).as_posix()}\n")
                if reference:
                    additions.append(f"Identical text reproduced above: {reference}\n")
                else:
                    additions.append(f"{content.rstrip()}\n")
                    seen_files[digest] = label
            elif path.suffix.lower() in {
                ".rs",
                ".c",
                ".h",
                ".cc",
                ".cpp",
                ".cxx",
                ".hh",
                ".hpp",
                ".hxx",
                ".m",
                ".mm",
                ".s",
                ".md",
                ".txt",
                ".py",
                ".sh",
                ".java",
                ".kt",
                ".flf",
            } or name.startswith(("readme", "authors", "contributors")):
                for line in path.read_text(
                    encoding="utf-8", errors="replace"
                ).splitlines():
                    if re.search(
                        r"copyright\s*(?:\([cC]\)|©|[12][0-9]{3})", line, re.IGNORECASE
                    ):
                        copyrights.add(line.strip())
        if copyrights:
            additions.append("\nCopyright statements preserved from upstream files:\n")
            additions.extend(f"{line}\n" for line in sorted(copyrights))
    NOTICES.write_text(
        header + text + "".join(additions), encoding="utf-8", newline="\n"
    )
    (ROOT / "THIRD-PARTY-NOTICES.sha256").write_text(
        hashlib.sha256(NOTICES.read_bytes()).hexdigest()
        + "  THIRD-PARTY-NOTICES.txt\n",
        encoding="utf-8",
    )
    print(f"Wrote {NOTICES.name}; corresponding source: {source_url}")


def check_notices(offline: bool) -> None:
    text = NOTICES.read_text(encoding="utf-8")
    if f"Cargo.lock SHA-256: {lock_digest()}\n" not in text:
        raise ValueError("Cargo.lock changed; regenerate THIRD-PARTY-NOTICES.txt")
    if f"Notice inputs SHA-256: {notice_inputs_digest()}\n" not in text:
        raise ValueError("Notice inputs changed; regenerate THIRD-PARTY-NOTICES.txt")
    recorded = (
        (ROOT / "THIRD-PARTY-NOTICES.sha256").read_text(encoding="utf-8").split()[0]
    )
    if recorded != hashlib.sha256(NOTICES.read_bytes()).hexdigest():
        raise ValueError("THIRD-PARTY-NOTICES.txt failed its integrity check")
    for package in metadata(offline)["packages"]:
        if (
            package["name"] not in {"rustiq-chem", "rustiq-chem-core"}
            and f"PACKAGE: {package['name']} {package['version']}\n" not in text
        ):
            raise ValueError(
                f"Missing notices for {package['name']} {package['version']}"
            )
    if (
        "GNU GENERAL PUBLIC LICENSE" not in text
        or "GNU LESSER GENERAL PUBLIC LICENSE" not in text
    ):
        raise ValueError("GPL/LGPL license texts are missing")
    print("Distribution notices match the locked workspace dependency graph")


def source_archive(offline: bool, output: Path) -> None:
    check_notices(offline)
    # Release sources must come from the same committed tree as the executable.
    status = run("git", "status", "--porcelain", "--untracked-files=all").splitlines()
    # dist's shell opens these output files before it starts the global build.
    generated_manifests = {"?? dist-manifest.json", "?? plan-dist-manifest.json"}
    if any(line not in generated_manifests for line in status):
        raise ValueError("Create source artifacts from a clean committed checkout")
    run(
        "git",
        "ls-files",
        "--error-unmatch",
        "DISTRIBUTION.md",
        "COPYING",
        "THIRD-PARTY-NOTICES.txt",
        "tools/distribution.py",
    )
    commit = run("git", "rev-parse", "HEAD").strip()
    output = output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="rustiq-source-") as temporary:
        stage = Path(temporary)
        source = stage / "rustiq-source"
        source.mkdir()
        repository_tar = stage / "repository.tar"
        subprocess.run(
            ["git", "archive", "--format=tar", "-o", str(repository_tar), "HEAD"],
            cwd=ROOT,
            check=True,
        )
        with tarfile.open(repository_tar) as archive:
            archive.extractall(source, filter="data")
        args = ["cargo", "vendor", "--locked", "--versioned-dirs", "vendor"]
        if offline:
            args.append("--offline")
        config = run(*args, cwd=source)
        cargo_config = source / ".cargo" / "config.toml"
        cargo_config.parent.mkdir(exist_ok=True)
        existing = (
            cargo_config.read_text(encoding="utf-8") if cargo_config.exists() else ""
        )
        if "[source." in existing:
            raise ValueError(
                "Review existing Cargo source replacement before creating the source archive"
            )
        cargo_config.write_text(existing + "\n" + config, encoding="utf-8")
        info = f"Repository commit: {commit}\nCargo.lock SHA-256: {lock_digest()}\n"
        (source / "SOURCE-INFO.txt").write_text(info, encoding="utf-8")
        # This also verifies that Cargo can resolve every target's dependencies
        # from the supplied vendor tree, without the registry or network.
        run(
            "cargo",
            "metadata",
            "--offline",
            "--locked",
            "--all-features",
            "--format-version",
            "1",
            cwd=source,
        )
        with tarfile.open(output, "w:gz") as archive:
            archive.add(source, arcname=source.name)
    print(f"Wrote {output}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["notices", "check", "source"])
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--cargo-about", default="cargo-about")
    parser.add_argument(
        "--output",
        type=Path,
        default=ROOT / "target/distrib/rustiq-corresponding-source.tar.gz",
    )
    args = parser.parse_args()
    if args.command == "notices":
        prepare_notices(args.offline, args.cargo_about)
    elif args.command == "check":
        check_notices(args.offline)
    else:
        source_archive(args.offline, args.output)


if __name__ == "__main__":
    main()
