# Reference Comparisons

This directory contains optional development tooling for comparing RustiQ sample
energies against PySCF. These checks are intentionally separate from `cargo test`
so the Rust test suite does not depend on Python or PySCF.

The Nix development shell provides the PySCF version locked in `uv.lock` on all
platforms declared by the flake. Inside the Dev Container, the same tools are
prepared automatically and are available directly on `PATH`. No `.venv` or
activation step is required.

From the Dev Container, run all reference comparisons with:

```sh
python tools/reference/compare_pyscf.py
```

From another Nix-capable environment, run:

```sh
nix develop --command python tools/reference/compare_pyscf.py
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
uv run --locked python tools/reference/compare_pyscf.py
```

This uses the cross-platform lock and requires compatible binary wheels; it
does not build Python packages from source. PySCF supports Linux, macOS, and
WSL2, but not native Windows. The comparison script also invokes `cargo`, so
Cargo must remain on `PATH`.

Run a single case by name:

```sh
uv run --locked python tools/reference/compare_pyscf.py h2-sto-3g-rhf
```

The open-shell, non-degenerate H₂⁺ UHF reference used by the Rust UHF test can
be reproduced with:

```sh
uv run --locked python tools/reference/compare_pyscf.py h2-plus-sto-3g-uhf
```

The equivalent flake app invocation is:

```sh
nix run .#pyscf-check -- h2-sto-3g-rhf
```

Prefix the single-case command with `nix develop --command` when running it
outside the Dev Container or an active Nix development shell.

The script prepares a temporary RustiQ basis store from `tests/data/sto-3g.json`
and does not download basis data for RustiQ. `uv.lock` records the wheels and
hashes used by both uv and Nix; update it intentionally with `uv lock` whenever
the Python dependency declarations change.

RustiQ is invoked with `run --format json`; the validator reads schema version 1
JSON directly, verifies that the HF calculation converged, and emits its report
with Python's CSV writer rather than parsing the human-readable terminal report.
This keeps reference validation independent of report wording and decimal
formatting.
