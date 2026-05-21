# RustiQ

RustiQ is a Rust-based quantum chemistry command-line application. It currently focuses on parsing molecular inputs, loading Gaussian basis sets, and running Hartree-Fock self-consistent field calculations.

## Features

- TOML run files for calculation setup
- XYZ molecular geometry parsing
- Local and online basis-set management
- Gaussian basis functions and integral routines
- Hartree-Fock SCF calculations with configurable convergence settings

## Requirements

- Rust stable with Cargo
- Network access when downloading basis sets

## Quick Start

Build the project:

```sh
cargo build
```

Run the test suite:

```sh
cargo test
```

Run the sample calculation:

```sh
cargo run -- run --file samples/h2/calculation.toml
```

The H2 sample TOML uses `samples/h2/molecule.xyz` by default because the CLI changes the working directory to the run file location before reading relative paths.

Run a heavier SCF/performance sample:

```sh
cargo run -- run --file samples/ethanol_sto3g_stress/calculation.toml
```

That sample includes `samples/ethanol_sto3g_stress/pyscf_reference.py` for an
optional PySCF comparison. It is not part of the Rust unit test suite.

## Input Files

A minimal calculation file looks like this:

```toml
[global]
basis = "sto-3g"

[global.molecule]
geometry = "./molecule.xyz"
charge = 0
multiplicity = 1
molecule_unit = "Angstrom"

[hf]
max_iterations = 100
convergence_threshold = 1e-8
density_guess = "CoreHamiltonian"
```

Available density guesses are `CoreHamiltonian`, `OneElectron`, `Zero`, `Random`, and
`RandomSymmetric`. If omitted, `OneElectron` is used.

The molecule file should use XYZ format:

```xyz
2
Hydrogen molecule
H 0.0 0.0 -0.37
H 0.0 0.0  0.37
```

## Basis Sets

List locally cached basis sets:

```sh
cargo run -- basis list
```

List online basis sets:

```sh
cargo run -- basis list --online
```

Download a basis set:

```sh
cargo run -- basis download sto-3g
```

Remove cached basis sets:

```sh
cargo run -- basis remove --names sto-3g
```

## Development

Format and lint before submitting changes:

```sh
cargo fmt
cargo clippy --all-targets --all-features
```

Most tests are colocated with implementation modules in `src/`. Shared fixtures live in `tests/data/`, and example calculation inputs live in `samples/`.
