# Reference Comparisons

This directory contains optional development tooling for comparing RustiQ sample
energies against PySCF. These checks are intentionally separate from `cargo test`
so the Rust test suite does not depend on Python or PySCF.

The Nix development shell provides PySCF on supported platforms. Inside the
Dev Container, the same tools are prepared automatically from `flake.lock` and
are available directly on `PATH`.

From the Dev Container, run all reference comparisons with:

```sh
python tools/reference/compare_pyscf.py
```

From another Nix-capable environment, run:

```sh
nix develop --command python tools/reference/compare_pyscf.py
```

Run a single case by name:

```sh
python tools/reference/compare_pyscf.py h2-sto-3g-rhf
```

Prefix the single-case command with `nix develop --command` when running it
outside the Dev Container or an active Nix development shell.

The script prepares a temporary RustiQ basis store from `tests/data/sto-3g.json`
and does not download basis data for RustiQ. In the nixpkgs revision pinned by
`flake.lock`, `python314Packages.pyscf` supports exactly `x86_64-linux` and
`aarch64-darwin`; it is unavailable on the flake's third platform,
`aarch64-linux`. The Dev Container uses `x86_64-linux`, so the reference
environment is available there.
