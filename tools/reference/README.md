# Reference Comparisons

This directory contains optional live PySCF comparisons of sample energies and
AO integrals, plus independent integral reference generation. Ordinary Cargo
unit tests compare production integrals with committed NumPy fixtures without
requiring Python or PySCF.

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
WSL2, but not native Windows. On native Windows, uv installs the other Python
tools and only tests requiring PySCF are skipped. NumPy persistence and report
validation tests still run; use WSL2 for PySCF comparisons and scientific report
generation. Cargo must remain on `PATH` when using the
default `cargo run` invocation.
Set `RUSTIQ_BIN` to an absolute executable path to compare a specific build
instead; `nix run .#pyscf-check` supplies its Nix-built executable this way.
The full directory suite also invokes Cargo for Rust/NumPy persistence
interoperability checks, even when `RUSTIQ_BIN` is set.

Select the H₂/STO-3G RHF comparison without its MP2 variant:

```sh
uv run --locked pytest tools/reference -k 'h2-sto-3g-rhf and not mp2'
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

The MP2 comparisons cover RHF H₂/STO-3G, H₂/cc-pVDZ, H₂O/6-31G, and
UHF OH/STO-3G with all occupied orbitals correlated (no frozen core). Each case checks both the HF
total energy and the MP2 correlation energy:

Additional H₂O/6-31G and OH/STO-3G comparisons freeze one occupied orbital and
exercise both 32 KiB and 512 MiB MP2 workspace budgets using temporary runfiles.
The smaller budget forces multiple occupied blocks for water.

```sh
uv run --locked pytest tools/reference -k mp2
```

The open-shell, non-degenerate H₂⁺ UHF reference used by the Rust UHF test can
be reproduced with:

```sh
uv run --locked pytest tools/reference -k h2-plus-sto-3g-uhf
```

The equivalent flake app invocation is:

```sh
nix run .#pyscf-check -- -k h2-plus-sto-3g-uhf
```

Prefix the single-case command with `nix develop --command` when running it
outside the Dev Container or an active Nix development shell.

The versioned store under `tests/data/reference/` contains unmodified basis files
downloaded from Basis Set Exchange with RustiQ. At session startup, pytest copies
the required files into a temporary store, so all comparisons run offline. To
refresh a fixture while keeping the source explicit, point
`RUSTIQ_DATA_BASIS` at `tests/data/reference/RustiQ/basis_sets` and run
`cargo run -- basis download <name>`.
`uv.lock` records the wheels and
hashes used by both uv and Nix; update it intentionally with `uv lock` whenever
the Python dependency declarations change.

Generate a full precision scientific validation report from the same cases and
comparisons:

```sh
uv run --locked python tools/reference/report.py --output-dir target/scientific-report
```

The command writes `validation.json` and `validation.md`. It includes the
canonical cases plus the two frozen-core MP2 cases at each memory budget used
by pytest. A failed or non-finite calculation remains visible in the report and
causes a nonzero exit status. The JSON contains machine-readable values;
Markdown formats energies for reading. The comparison only validates selected
numerical cases against PySCF. It does not establish general chemical accuracy
or make HF/MP2 reliable for every system. Matching thresholds also do not imply
identical convergence criteria.

RustiQ is invoked with `run --format json`; pytest reads schema version 1 JSON
directly, checks convergence, methods, and energies, and provides all comparison
reporting. This keeps reference validation independent of terminal report wording
and decimal formatting. `compare_pyscf.py` owns the reference case definitions
and shared comparison functions. Its command-line entry point forwards arguments
to pytest for compatibility; use the commands above for new workflows.

Independent AO integral tests are in `test_integrals.py`; the corresponding
Cargo unit test runs without Python against versioned NumPy fixtures under
`crates/rustiq-core/tests/data/integrals/`. NumPy generates the arrays and Rust
loads them using `npyz`. Regenerate with:

```sh
uv run --locked python tools/reference/integrals.py --regenerate
```

The generator uses only PySCF and raw BSE source data. Pytest verifies the
stored arrays and compares live production Rust integrals against PySCF.
See the manual’s [Independent AO integral validation](../../docs/src/development/contributing.md#independent-ao-integral-validation) section for AO mapping,
normalization factors, tolerances, coverage, and limitations. The existing
scientific report includes the integral metrics.
