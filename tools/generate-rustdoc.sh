#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
open_args=()

case "${1:-}" in
    "")
        ;;
    --open)
        open_args=(--open)
        ;;
    *)
        echo "Usage: $0 [--open]" >&2
        exit 2
        ;;
esac

cd "$repo_root"
cargo rustdoc --package rustiq-core "${open_args[@]}" -- \
    --html-in-header "$repo_root/crates/rustiq-core/docs/rustdoc-mathjax.html"
