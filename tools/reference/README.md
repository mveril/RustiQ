# Reference Comparisons

This directory contains optional development tooling for comparing RustiQ sample
energies against PySCF. These checks are intentionally separate from `cargo test`
so the Rust test suite does not depend on Python or PySCF.

The Nix development shell provides the PySCF version locked in `uv.lock` on all
platforms declared by the flake. Inside the Dev Container, the same tools are
prepared automatically and are available directly on `PATH`. No `.venv` or
activation step is required.

From the Dev Container, run all reference comparisons with the canonical interface:

```sh
pytest tools/reference
```

Outside the Dev Container, the minimal Nix shell includes the same test group:

```sh
nix develop .#mini-pyscf --command pytest tools/reference
```

From another Nix-capable environment, run:

```sh
nix develop --command pytest tools/reference
```

The flake also exposes a manual app that supplies the Nix-built RustiQ binary,
Python, and PySCF without entering the development shell. Run it from the
repository root:

```sh
nix run .#pyscf-check
```

This app is deliberately not part of the flake's `checks`, so `nix flake check`
does not run the PySCF comparisons automatically. PySCF is built from its
binary wheel through `uv2nix`, independently of whether nixpkgs packages it for
the current platform.

Without Nix, install the Rust toolchain described in the root `README.md` and
[`uv`](https://docs.astral.sh/uv/). Then run:

```sh
uv run --locked pytest tools/reference
```

This uses the cross-platform lock and requires compatible binary wheels; it
does not build Python packages from source. PySCF supports Linux, macOS, and
WSL2, but not native Windows. The comparison script also invokes `cargo`, so
Cargo must remain on `PATH`.

Run a single case by its pytest parameter ID:

```sh
uv run --locked pytest tools/reference -k h2-sto-3g-rhf
```

The H₂ matrix also covers the split-valence 6-31G and correlation-consistent
cc-pVDZ basis sets:

```sh
uv run --locked pytest tools/reference -k 'h2-6-31g-rhf or h2-cc-pvdz-rhf'
```

Water exercises a polyatomic, polar geometry and oxygen s/p contractions in all
three basis families, plus spherical d polarization functions with cc-pVDZ:

```sh
uv run --locked pytest tools/reference -k h2o
```

The open-shell, non-degenerate H₂⁺ UHF reference used by the Rust UHF test can
be reproduced with:

```sh
uv run --locked pytest tools/reference -k h2-plus-sto-3g-uhf
```

The equivalent flake app invocation is:

```sh
nix run .#pyscf-check -- -k h2-sto-3g-rhf
```

Prefix the single-case command with `nix develop --command` when running it
outside the Dev Container or an active Nix development shell.

The versioned store under `tests/data/reference/` contains unmodified basis files
downloaded from Basis Set Exchange with RustiQ. At session startup, pytest copies
the required files into a temporary store, so all comparisons run offline. To
refresh a fixture while keeping the source explicit, point `RUSTIQ_DATA_HOME` at
`tests/data/reference` and run `cargo run -- basis download <name>`.
`uv.lock` records the wheels and
hashes used by both uv and Nix; update it intentionally with `uv lock` whenever
the Python dependency declarations change.

RustiQ is invoked with `run --format json`; pytest reads schema version 1 JSON
directly, checks convergence, methods, and energies, and provides all comparison
reporting. This keeps reference validation independent of terminal report wording
and decimal formatting. `python tools/reference/compare_pyscf.py` remains a thin
compatibility launcher that forwards its arguments to pytest.
