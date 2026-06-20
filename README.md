# RustiQ

RustiQ is a Rust prototype for quantum chemistry software.

[![CI](https://github.com/mveril/RustiQ/actions/workflows/ci.yml/badge.svg)](https://github.com/mveril/RustiQ/actions/workflows/ci.yml)

License: MIT OR Apache-2.0.

The point of this repository is not to claim that a young Rust code can already
replace Gaussian, Molpro, PySCF, Psi4, ORCA, or Quantum Package. The point is to
show what a modern electronic-structure codebase can look like when it is built
with current software-engineering tools: strong types, explicit errors,
structured input files, unit and CLI tests, reproducible dependencies, readable
diagnostics, and a small architecture that a new contributor can actually
understand.

This README is written for two audiences:

- theoretical chemists who are used to mature Fortran-era codes and care first
  about equations, numerical validation, and scientific trust;
- software engineers working on scientific codes who care about maintainability,
  modularity, developer experience, dependency management, and how much
  accidental complexity a codebase accumulates over time.

## Current Status

RustiQ is currently an experimental command-line application. It can parse TOML
calculation files, read XYZ molecular geometries, load Gaussian basis sets,
construct molecular integrals, run Hartree-Fock calculations, and optionally run
MP2 from converged HF orbitals.

Implemented today:

- `clap`-based command-line interface;
- TOML runfiles parsed through `toml-spanner`;
- source-span diagnostics for invalid input through `miette`;
- XYZ geometry parsing and unit conversion;
- geometry inspection and transformation commands for info, rotation,
  translation, centering, and combined isometries;
- molecule validation for electron count, charge, and multiplicity;
- local basis-set cache and optional online basis-set download;
- Gaussian basis construction;
- one-electron core Hamiltonian terms;
- overlap matrix construction and symmetric orthogonalization;
- compact two-electron integral storage;
- RHF for closed-shell systems;
- UHF for open-shell systems;
- Automatic RHF/UHF resolution;
- DIIS acceleration;
- multiple density guesses, including core-Hamiltonian and randomized guesses;
- finite-value and positive-definiteness checks around sensitive numerical
  operations;
- RHF-MP2 and UHF-MP2 correlation energy paths;
- CLI reports for SCF and MP2;
- sample calculations for H2, OH, ethanol, and cholesterol;
- unit tests and CLI sample tests.

This is not yet production scientific software. The numerical results must be
treated as experimental and cross-checked against established programs before
being used for chemical interpretation.

Do not use RustiQ results for research conclusions without independent
validation against established quantum chemistry packages.

## What Matters In The Codebase

The repository is intentionally split into small domains:

- `src/cli/` handles command dispatch, terminal output, and user-facing reports.
- `src/runfile/` owns the TOML input schema, validation, typed configuration, and
  diagnostics.
- `src/molecules/` owns atoms, elements, geometry parsing, units, charge,
  multiplicity, electron-count logic, and geometry transforms.
- `src/basis/` owns basis-set files, cache management, Gaussian shells, and
  contractions.
- `src/eri.rs` and `src/eri/` own electron-repulsion integrals and compact ERI
  indexing/storage.
- `src/hf/` owns RHF, UHF, DIIS, density guesses, numerical checks, and SCF
  results.
- `src/mp2.rs` owns the post-HF MP2 layer for RHF and UHF references.
- `tests/cli_samples.rs` checks that real command-line samples run and produce
  expected energies.

This separation is one of the core messages of the project. A quantum chemistry
code does not have to be a single historical mass of tightly coupled routines.
It can expose clear boundaries between input, chemistry objects, basis handling,
integrals, SCF algorithms, post-HF methods, and reporting.

## Why Rust Is Interesting Here

Rust is not automatically better than Fortran for numerical kernels. Mature
Fortran code is still extremely good at dense numerical work, and many
electronic-structure packages exist because Fortran made high-performance
scientific programming possible.

The interesting argument for Rust is different:

- Rust makes ownership and mutation explicit, which helps when large tensors,
  matrices, caches, and temporary workspaces start interacting.
- Typed errors with `thiserror`, `anyhow`, and `miette` make failure modes part
  of the design instead of an afterthought.
- Cargo makes dependency management, testing, feature flags, formatting, and
  reproducible builds standard rather than project-specific infrastructure.
- Pattern matching and enums make configuration states explicit, for example HF
  method selection, density guesses, and validated runfile options.
- The ecosystem allows the project to reuse maintained crates instead of
  rebuilding every tool from scratch.
- Safe Rust is a strong default for high-level orchestration, while unsafe code
  can be isolated and documented when performance-sensitive storage needs it.

In RustiQ, this shows up concretely in the compact ERI storage, typed runfile
validation, DIIS configuration, UHF spin handling, MP2 input validation, and
source-located diagnostics for input files and xyz files.

## Differentiators From A Software Engineering View

RustiQ deliberately uses community crates where they make the code clearer:

- `nalgebra` for dense matrices, linear algebra, and 3D geometry operations;
- `ndarray` for array-shaped reference data in compact tensor tests;
- `rayon` for data parallelism in integral and post-HF paths;
- `clap` for declarative command-line parsing;
- `serde`, `serde_json`, and `toml-spanner` for structured data and recoverable
  TOML parsing;
- `miette` for diagnostics that point at invalid TOML fields and XYZ geometry
  lines;
- `thiserror` for explicit error handling;
- `reqwest`, `tokio`, `dirs`, and `indicatif` for optional online basis-set download
  and cache behavior;
- `periodic_table` and `physical_constants` rather than hand-maintained
  chemistry constants;
- `rstest`, `proptest`, and `approx` for numerical and property-style testing;
- `tabled`, `humantime`, `figlet-rs`, and `bat` for readable terminal output.

For a software engineer, the important point is not that every dependency is
final. The point is that the project is designed like a modern application:
dependency graph declared in one place, lockfile committed, tests integrated,
formatting standardized, features configurable, and errors surfaced cleanly.

This also makes the architecture easier to compare with established research
codes: scientific kernels can be isolated from user-interface and input-code
paths, low-level optimizations can be concentrated in small modules such as
compact ERI storage, and the code structure makes it clear where more serious
Rust-native integral engines, determinant machinery, or perturbative corrections
could be plugged in later.
The extensibility is mainly visible for density guess and random distribution.

## What Theoretical Chemists May Appreciate

For a theoretical chemist or a thesis supervisor, the interesting part is that
the computational chemistry concepts are visible rather than hidden behind a
large legacy interface:

- the runfile explicitly states basis, molecule, charge, multiplicity, HF method,
  convergence threshold, DIIS, density guess, and MP2 options;
- RHF and UHF paths are separate enough to discuss the physical assumptions;
- MP2 is implemented as a post-HF layer that depends on converged HF orbitals;
- the code checks that MP2 is not run on an unconverged HF result;
- open-shell examples resolve to UHF and are tested through the CLI;
- numerical failure modes are not only strings; finite values, dimensions,
  orbital partitions, and overlap positive-definiteness are checked explicitly;
- sample outputs can be compared to reference packages such as PySCF;
- the codebase is small enough that SCF, UHF, ERIs, and MP2 can be located
  quickly and discussed directly;
- the separation between equations, inputs, validation, algorithms, and reports
  makes the scientific assumptions inspectable;
- the project leaves room for didactic method implementations without
  immediately fighting a large historical infrastructure.

This makes RustiQ useful as a discussion object for people used to mature
Fortran-era packages as well as for developers of research codes: one can inspect
where a method is implemented, where the assumptions enter, where validation is
missing, and where performance would need serious work.

The useful conversation is not "Rust versus Fortran" in the abstract. RustiQ's
chemistry implementation is written in Rust: portable by default, easy to build
on the main desktop and HPC platforms, and still performance-oriented through
Rust's native compilation model, `rayon` parallelism, and `nalgebra`-based
numerical linear algebra. The goal is to show that a clean quantum chemistry
code can keep both the scientific layers and the performance-critical
implementation in the same modern, cross-platform ecosystem.

## What Is Still Missing For Research Use

The missing pieces are substantial:

- systematic validation against established codes across molecules, bases, spin
  states, and charge states;
- a documented numerical reference suite with tolerances;
- broader basis-set support, angular-momentum coverage, contraction conventions,
  and normalization validation;
- gradients, geometry optimization, and vibrational frequencies;
- DFT functionals, grids, and numerical integration;
- robust treatment of larger systems with explicit memory strategy;
- faster Rust-native integral algorithms, screening, batching, and memory-aware
  integral handling;
- more post-HF methods and stronger validation of the current MP2 layer;
- standard chemistry formats beyond the current TOML/XYZ workflow;
- scientific documentation of equations, conventions, units, and tested
  approximations;
- benchmarks against PySCF, Psi4, ORCA, Quantum Package, and other relevant
  references;
- release packaging, versioned documentation, and a stable CLI contract.

Until those are addressed, RustiQ should be described as a modern prototype and
architecture experiment, not as a production research code.

## Quick Start

Build the project:

```sh
cargo build
```

Run the test suite:

```sh
cargo test
```

Run a simple Hartree-Fock calculation:

```sh
cargo run -- run samples/h2/sto-3g/calculation.toml
```

Run an MP2 example:

```sh
cargo run -- run samples/h2/sto-3g/mp2_calculation.toml
```

Run an open-shell UHF example:

```sh
cargo run -- run samples/oh/sto-3g/calculation.toml
```

Run a larger sample:

```sh
cargo run -- run samples/ethanol/sto-3g/calculation.toml
```

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
method = "Rhf"
max_iterations = 100
convergence_threshold = 1e-8
diis = true
diis_size = 8

[hf.guess]
type = "CoreHamiltonian"
```

An MP2 calculation adds:

```toml
[mp2]
frozen_orbitals = 0
```

The molecule file uses XYZ format:

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
cargo run -- basis remove sto-3g
```

## Development

Useful checks before submitting a change:

```sh
cargo fmt
cargo clippy --all-targets --all-features
cargo test
```

Most unit tests are colocated with implementation modules in `src/`. Shared
fixtures live in `tests/data/`, and sample calculation inputs live in
`samples/`.

See also:

- `CONTRIBUTING.md` for contribution guidelines.
- `ROADMAP.md` for project priorities and research-grade requirements.
- `CITATION.cff` for citation metadata.
