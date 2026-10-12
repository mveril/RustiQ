# Ethanol STO-3G Stress Sample

This sample is intentionally larger than the default H2 and H2O examples. It uses
ethanol with STO-3G, giving 21 contracted Cartesian basis functions instead of 2
for H2. The tighter convergence threshold and larger DIIS subspace exercise a
more demanding SCF configuration. Iteration count and runtime depend on the
initial guess, convergence behavior, and execution environment.

Run these commands from the repository root. Make STO-3G available offline
using the checked-in fixture, then use an optimized build:

```sh
cargo run --locked -- basis import tests/data/sto-3g.json
cargo run --release --locked -- run samples/ethanol/sto-3g/calculation.toml
```

The command is the same inside the Dev Container; no host Rust installation is
required there.

Run the optional PySCF reference outside the Rust unit tests:

```sh
uv run --locked python samples/ethanol/sto-3g/pyscf_reference.py
```

The locked Python environment supports Linux, macOS, and WSL2. In
`nix develop .#mini-pyscf`, the full Nix shell, or the Dev Container, Python and
PySCF are already on `PATH`, so use:

```sh
python samples/ethanol/sto-3g/pyscf_reference.py
```

This helper prints a separate PySCF RHF total energy and convergence diagnostics;
it does not automatically compare the RustiQ result or define an acceptance
tolerance. The [canonical reference suite](../../../tools/reference/README.md)
provides automated comparisons for its listed cases. Ethanol remains a stress
sample, not an independently validated reference case in that suite.
