#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$repo_root"
cargo rustdoc --package rustiq-core -- \
    --html-in-header "$repo_root/docs/rustdoc-mathjax.html"
