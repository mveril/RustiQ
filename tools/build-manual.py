#!/usr/bin/env python3
"""Build the relocatable offline manual (run from the repository root)."""

import argparse
import hashlib
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path
from urllib.request import urlopen

MATHJAX_URL = "https://registry.npmjs.org/mathjax/-/mathjax-2.7.9.tgz"
MATHJAX_SHA256 = "7131e739848edc14aa661a5516995866b81a477fab8b039d7cc324930e71f786"


def install_mathjax(archive: Path, destination: Path) -> None:
    if hashlib.sha256(archive.read_bytes()).hexdigest() != MATHJAX_SHA256:
        raise ValueError("MathJax archive SHA-256 mismatch")
    destination.mkdir(parents=True, exist_ok=True)
    with tarfile.open(archive) as bundle:
        for member in bundle.getmembers():
            parts = Path(member.name).parts
            if len(parts) < 2 or parts[0] != "package":
                continue
            if parts[1] not in {
                "MathJax.js",
                "config",
                "extensions",
                "jax",
                "fonts",
                "localization",
                "LICENSE",
                "LICENSE.txt",
                "README.md",
            }:
                continue
            target = destination.joinpath(*parts[1:])
            if not target.resolve().is_relative_to(destination.resolve()):
                raise ValueError(f"unsafe MathJax path: {member.name}")
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
            elif member.isfile():
                target.parent.mkdir(parents=True, exist_ok=True)
                with bundle.extractfile(member) as source, target.open("wb") as output:
                    shutil.copyfileobj(source, output)
            else:
                raise ValueError(f"unsupported MathJax member: {member.name}")


def build(archive: Path) -> None:
    subprocess.run(["mdbook-mermaid", "install", "docs"], check=True)
    subprocess.run(["mdbook", "build", "docs"], check=True)
    book = Path("docs/book")
    install_mathjax(archive, book / "mathjax")
    loaders = 0
    for page in book.rglob("*.html"):
        prefix = "../" * (len(page.relative_to(book).parts) - 1)
        html, replacements = re.subn(
            r'https://cdnjs\.cloudflare\.com/ajax/libs/mathjax/[^"<>]+',
            f"{prefix}mathjax/MathJax.js?config=TeX-AMS-MML_SVG",
            page.read_text(encoding="utf-8"),
        )
        loaders += replacements
        # mdBook's web-only 404 base otherwise points outside a moved file tree.
        html = re.sub(r'<base href="[^"]*">', '<base href="./">', html)
        page.write_text(html, encoding="utf-8")
    if not loaders:
        raise ValueError("no mdBook MathJax loaders found")
    subprocess.run(
        [
            sys.executable,
            "tools/check-book-links.py",
            str(book),
            "--config",
            "docs/book.toml",
            "--offline",
        ],
        check=True,
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--mathjax-archive",
        type=Path,
        help="use a pinned local archive without downloading",
    )
    args = parser.parse_args()
    if args.mathjax_archive:
        build(args.mathjax_archive)
    else:
        with tempfile.TemporaryDirectory(prefix="rustiq-mathjax-") as temporary:
            archive = Path(temporary) / "mathjax.tgz"
            with (
                urlopen(MATHJAX_URL, timeout=60) as source,
                archive.open("wb") as output,
            ):
                shutil.copyfileobj(source, output)
            build(archive)


if __name__ == "__main__":
    main()
