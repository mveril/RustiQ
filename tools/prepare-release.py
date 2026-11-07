"""Create a local release tag with Nix release metadata, then resume development."""

import argparse
import re
import subprocess
import tomllib
from pathlib import Path

TAG_PATTERN = r"v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?"
DEVELOPMENT_FLAG = "      officialRelease = false;"
RELEASE_FLAG = "      officialRelease = true;"


def git(root, *args):
    return subprocess.check_output(["git", "-C", str(root), *args], text=True).strip()


def prepare(root, tag):
    if not re.fullmatch(TAG_PATTERN, tag):
        raise ValueError("release tag must match vX.Y.Z or vX.Y.Z-prerelease")
    branch = git(root, "branch", "--show-current")
    if not re.fullmatch(r"[a-z]+/[a-z0-9]+(?:-[a-z0-9]+)*", branch):
        raise ValueError(
            "prepare releases on a type/lowercase-description branch; main is protected"
        )
    if git(root, "status", "--porcelain", "--untracked-files=all"):
        raise ValueError("commit or stash local changes before preparing a release")
    if git(root, "tag", "--list", tag):
        raise ValueError(f"tag {tag} already exists")
    version = tag[1:]
    for manifest in (root / "Cargo.toml", root / "crates/rustiq-core/Cargo.toml"):
        document = tomllib.loads(manifest.read_text())
        if document["package"]["version"] != version:
            raise ValueError(f"{manifest}: package version must match {version}")
    document = tomllib.loads((root / "Cargo.toml").read_text())
    requirement = document["dependencies"]["rustiq-core"]["version"]
    if requirement.removeprefix("=") != version:
        raise ValueError("rustiq-core dependency version must match the release")
    flake = root / "flake.nix"
    development = flake.read_text()
    if development.count(DEVELOPMENT_FLAG) != 1 or RELEASE_FLAG in development:
        raise ValueError(
            "flake.nix must contain exactly one disabled officialRelease flag"
        )

    flake.write_text(development.replace(DEVELOPMENT_FLAG, RELEASE_FLAG))
    git(root, "add", "flake.nix")
    git(root, "commit", "-m", f"Enable Nix release metadata for {tag}")
    release_commit = git(root, "rev-parse", "HEAD")
    flake.write_text(development)
    git(root, "add", "flake.nix")
    git(root, "commit", "-m", f"Resume Nix development builds after {tag}")
    git(root, "tag", tag, release_commit)
    print(f"Created local tag {tag}; {branch} has resumed development mode.")
    print(f"Push the preparation branch: git push -u origin {branch}")
    print(f"Open a pull request: gh pr create --base main --head {branch}")
    print("Merge the pull request preserving commits (no squash or rebase).")
    print("After the merge, fetch main and verify the tagged commit is included:")
    print("git fetch origin main")
    print(f"git merge-base --is-ancestor refs/tags/{tag} origin/main")
    print(f"Only after verification succeeds, publish: git push origin refs/tags/{tag}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tag", help="release tag, such as v1.2.3")
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    try:
        prepare(root, args.tag)
    except (ValueError, subprocess.CalledProcessError) as error:
        parser.exit(
            1,
            f"Release preparation failed: {error}\nInspect git status before retrying.\n",
        )


if __name__ == "__main__":
    main()
