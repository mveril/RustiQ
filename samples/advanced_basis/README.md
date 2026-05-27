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

Install the required basis files before running them:

```sh
cargo run -- basis download 6-31g
cargo run -- basis download cc-pvdz
```

Then run any sample with:

```sh
cargo run -- run samples/h2o/6-31g/calculation.toml
```

The `cc-pvdz` ethanol case is intentionally heavier than the STO-3G stress
sample and is useful for checking behavior with polarization functions and a
larger SCF problem.

The benzene `cc-pvdz` case uses a PubChem 3D conformer and is intended as a
heavy stress calculation that still fits the current dense ERI implementation.
