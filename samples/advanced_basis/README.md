# Advanced Basis Samples

These samples reuse the same molecules as the basic examples, but run them with
larger Gaussian basis sets:

- `samples/h2/6-31g/calculation.toml`
- `samples/h2/cc-pvdz/calculation.toml`
- `samples/h2o/6-31g/calculation.toml`
- `samples/h2o/cc-pvdz/calculation.toml`
- `samples/ethanol/6-31g/calculation.toml`
- `samples/ethanol/cc-pvdz/calculation.toml`
- `samples/benzene/cc-pvdz/calculation.toml`

Run these commands from the repository root. Install the required basis files
before running them; downloads require the CLI's `online` feature and network
access:

```sh
cargo run --locked -- basis download 6-31g
cargo run --locked -- basis download cc-pvdz
```

These commands work unchanged in the Dev Container. Open the repository with
**Dev Containers: Reopen in Container** first if you do not have the pinned Rust
and Nix toolchain installed locally.

Then run any sample with:

```sh
cargo run --release --locked -- run samples/h2o/6-31g/calculation.toml
```

The `cc-pvdz` ethanol case is intentionally heavier than the STO-3G stress
sample and is useful for checking behavior with polarization functions and a
larger SCF problem.

The benzene `cc-pvdz` case uses a PubChem 3D conformer and is intended as a
heavy stress calculation. RustiQ uses compact, symmetry-packed AO ERI storage,
whose size still grows as the fourth power of the basis dimension. Feasibility
depends on available memory; these samples do not establish a memory-fit or
performance guarantee. See the [scientific limitations](../../docs/src/scientific-scope.md).
