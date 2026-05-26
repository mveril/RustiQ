# Ethanol STO-3G Stress Sample

This sample is intentionally larger than the default H2 and H2O examples. It uses
ethanol with STO-3G, giving 21 contracted Cartesian basis functions instead of 2
for H2. DIIS is enabled with a larger subspace and the convergence threshold is
tighter so the SCF loop does more iterations and exposes more of the integral/Fock
parallel work.

Run RustiQ:

```sh
cargo run -- run --file samples/ethanol/sto-3g/calculation.toml
```

Run the optional PySCF reference outside the Rust unit tests:

```sh
pyscf-python samples/ethanol/sto-3g/pyscf_reference.py
```

RustiQ expects `sto-3g` to be present in the local basis store. If needed:

```sh
cargo run -- basis download sto-3g
```
