#!/usr/bin/env python3
"""Assemble a persistent GitHub Pages site from a relocatable mdBook."""

import argparse
import filecmp
import html
import tempfile
import json
import re
import shutil
import sys
from pathlib import Path

PATTERN = re.compile(
    r"^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$"
)


def version_key(version):
    match = PATTERN.fullmatch(version)
    if not match:
        raise ValueError(f"invalid release version: {version}")
    pre = match.group(4)
    identifiers = pre.split(".") if pre else []
    if any(x.isdigit() and len(x) > 1 and x[0] == "0" for x in identifiers):
        raise ValueError(f"invalid release version: {version}")
    return (
        *map(int, match.group(1, 2, 3)),
        0 if pre else 1,
        tuple((0, int(x)) if x.isdigit() else (1, x) for x in identifiers),
    )


def same_tree(a, b):
    left = {p.relative_to(a) for p in a.rglob("*") if p.is_file()}
    right = {p.relative_to(b) for p in b.rglob("*") if p.is_file()}
    return left == right and all(filecmp.cmp(a / p, b / p, shallow=False) for p in left)


def copy_tree(source, target):
    if target.exists():
        shutil.rmtree(target)
    shutil.copytree(source, target)


def prepare(book, target, name, release):
    copy_tree(book, target)
    # Nix store files are read-only; the website copy must be writable.
    for path in [target, *target.rglob("*")]:
        path.chmod(
            path.stat().st_mode | 0o700
            if path.is_dir()
            else path.stat().st_mode | 0o600
        )
    # Only the known mdBook edit-link prefix and offline 404 base are changed.
    for page in target.rglob("*.html"):
        content = page.read_text(encoding="utf-8")
        if release:
            content = content.replace(
                "https://github.com/mveril/RustiQ/edit/main/docs/",
                f"https://github.com/mveril/RustiQ/edit/{name}/docs/",
            )
        if page.name == "404.html" and page.parent == target:
            content = content.replace(
                '<base href="./">', f'<base href="/RustiQ/{name}/">'
            )
        page.write_text(content, encoding="utf-8")


def publish(book, site, revision, version=None):
    if not (book / "index.html").is_file():
        raise ValueError("book must contain index.html")
    if not re.fullmatch(r"[0-9a-f]{40}|[0-9a-f]{64}", revision):
        raise ValueError("source revision must be a full lowercase Git commit ID")
    if version:
        version_key(version)
    index = site / "versions.json"
    metadata = json.loads(index.read_text()) if index.exists() else {"versions": []}
    entries = metadata["versions"]
    for entry in entries:
        version_key(entry["version"])
    name = version or "dev"
    with tempfile.TemporaryDirectory() as tmp:
        prepared = Path(tmp) / "book"
        prepare(book, prepared, name, bool(version))
        dest = site / name
        old = (
            next((e for e in entries if e["version"] == version), None)
            if version
            else None
        )
        if version and (old or dest.exists()):
            if (
                not old
                or old["commit"] != revision
                or not dest.is_dir()
                or not same_tree(prepared, dest)
            ):
                raise ValueError(
                    f"version conflict for {version}: existing commit or content differs"
                )
        if version and not old:
            entries.append(
                {
                    "version": version,
                    "url": f"{version}/",
                    "channel": (
                        "beta" if re.search(r"-beta(?:\.|$)", version) else "prerelease"
                    )
                    if "-" in version
                    else "stable",
                    "commit": revision,
                }
            )
        entries.sort(key=lambda e: version_key(e["version"]), reverse=True)
        beta = next((e for e in entries if e["channel"] == "beta"), None)
        stable = next((e for e in entries if e["channel"] == "stable"), None)

        # Migrate existing root pages once into redirects to dev. Existing assets
        # are discarded; subsequent publications never overlay the root.
        legacy = {page.relative_to(prepared) for page in prepared.rglob("*.html")}
        if not metadata.get("legacy_redirects") and site.exists():
            for page in site.rglob("*.html"):
                relative = page.relative_to(site)
                if relative.parts[0] not in {
                    ".git",
                    "dev",
                    "beta",
                    "stable",
                } and not PATTERN.fullmatch(relative.parts[0]):
                    legacy.add(relative)
        site.mkdir(parents=True, exist_ok=True)
        if not metadata.get("legacy_redirects"):
            for child in site.iterdir():
                if child.name not in {
                    ".git",
                    "dev",
                    "beta",
                    "stable",
                } and not PATTERN.fullmatch(child.name):
                    if child.is_dir():
                        shutil.rmtree(child)
                    else:
                        child.unlink()
            for relative in legacy:
                if relative != Path("index.html"):
                    redirect(
                        site / relative,
                        "../" * (len(relative.parts) - 1)
                        + "dev/"
                        + relative.as_posix(),
                    )
        if not version or not old:
            copy_tree(prepared, dest)
        for alias, entry in [("beta", beta), ("stable", stable)]:
            if entry:
                alias_dir = site / alias
                if alias_dir.exists():
                    shutil.rmtree(alias_dir)
                redirect(alias_dir / "index.html", f"../{entry['version']}/")
        redirect(site / "index.html", "dev/")
        output = {
            "legacy_redirects": True,
            "development": "dev/",
            "latest_beta": beta["version"] if beta else None,
            "latest_stable": stable["version"] if stable else None,
            "versions": entries,
        }
        index.write_text(json.dumps(output, indent=2) + "\n", encoding="utf-8")


def redirect(path, target):
    path.parent.mkdir(parents=True, exist_ok=True)
    url = html.escape(target, quote=True)
    path.write_text(
        f'<!doctype html>\n<meta http-equiv="refresh" content="0; url={url}">\n'
        f'<title>Manual moved</title>\n<a href="{url}">Continue to the manual</a>\n',
        encoding="utf-8",
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("book", type=Path)
    parser.add_argument("site", type=Path)
    parser.add_argument("revision")
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--version")
    group.add_argument("--development", action="store_true")
    args = parser.parse_args()
    try:
        publish(args.book, args.site, args.revision, args.version)
    except (ValueError, OSError, KeyError) as error:
        parser.exit(1, f"publication failed: {error}\n")


if __name__ == "__main__":
    main()
