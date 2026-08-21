# Ethanol STO-3G Stress Sample

This sample is intentionally larger than the default H2 and H2O examples. It uses
ethanol with STO-3G, giving 21 contracted Cartesian basis functions instead of 2
for H2. DIIS is enabled with a larger subspace and the convergence threshold is
tighter so the SCF loop does more iterations and exposes more of the integral/Fock
parallel work.

Run RustiQ:

```sh
cargo run -- run samples/ethanol/sto-3g/calculation.toml
```

The command is the same inside the Dev Container; no host Rust installation is
required there.

Run the optional PySCF reference outside the Rust unit tests:

```sh
python samples/ethanol/sto-3g/pyscf_reference.py
```

Python with PySCF is provided automatically by the pinned Nix environment on
the Dev Container's `x86_64-linux` platform. Outside that environment, install
PySCF separately or enter the repository with `nix develop` first.

RustiQ expects `sto-3g` to be present in the local basis store. If needed:

```sh
cargo run -- basis download sto-3g
```
