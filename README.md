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
- Typed errors with `thiserror` and `miette` make failure modes part
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

### Recommended: VS Code Dev Container

The repository includes a ready-to-use development container. This is the
recommended setup when using VS Code because it provides the same Linux-based
Rust, Nix, debugging, and scientific Python tools without requiring them to be
installed individually on the host.

Prerequisites:

- Docker Desktop with the Linux engine running;
- Visual Studio Code;
- the VS Code **Dev Containers** extension.

Clone the repository and open it in VS Code:

```sh
git clone https://github.com/mveril/RustiQ.git
cd RustiQ
code .
```

Then run **Dev Containers: Reopen in Container** from the command palette. The
first creation can take several minutes while Docker builds the image and Nix
downloads the toolchain pinned by `flake.lock`. Subsequent starts reuse the
Docker image and Nix store.

The container is based on Debian Bookworm and installs only Nix and direnv at
the system level. The FlakeEnv extension loads `devShells.default` directly and
propagates its environment to terminals, tasks, debuggers, and language
servers. Rust, Nix, TOML, dependency, Python, Jupyter, and LLDB support is
installed as a small explicit extension list rather than through extension
packs with overlapping behavior.

The Rust toolchain, rust-analyzer, nixd, nixfmt, Ruff, scientific Python stack,
and development utilities remain pinned by `flake.lock`. The first activation
can take several minutes; later starts reuse the persistent Nix store.
Because `.envrc` execution requires explicit trust, run `direnv allow` once in
the container if FlakeEnv reports that it is blocked, then run **FlakeEnv:
Reload Environment**.

Verify the environment from a terminal inside the container:

```sh
cargo --version
rustc --version
rust-analyzer --version
cargo test
```

After changing `.devcontainer/devcontainer.json`, rebuild the container. After
changing `flake.nix` or `flake.lock`, run **FlakeEnv: Reload Environment**; a
container rebuild is only needed when the Dev Container configuration changes.

#### Dev Container Troubleshooting On Windows

- If Docker reports that `dockerDesktopLinuxEngine` cannot be found, start
  Docker Desktop and wait until `docker info` succeeds before reopening the
  repository.
- If container creation fails while mounting a path such as
  `\\wsl.localhost\<distribution>\mnt\wslg\runtime-dir\wayland-0`, disable
  **Dev Containers: Mount Wayland Socket** in the VS Code user settings. The
  equivalent JSON setting is `"dev.containers.mountWaylandSocket": false`.
  This only disables Linux GUI forwarding; it does not affect Docker, Rust,
  Nix, or terminal access.
- Nix inside the container is installed by the Dev Container feature. It is
  independent of any Nix or NixOS installation in WSL.
- If a flake change is not visible in the editor, run **FlakeEnv: Reload
  Environment** and restart the affected language server if necessary.

### Choosing A Development Environment

RustiQ can be developed with the VS Code Dev Container, directly with the
repository's Nix flake, or with a regular Rust installation. `direnv` is
optional outside the container: it automates entering and leaving the Nix
development shell.

If Nix is new to you, start with the official [introduction to
Nix](https://nixos.org/why-nix/) and [learning resources](https://nixos.org/learn/).
Nix is a package manager and development-environment tool; NixOS is a complete
Linux distribution built around it. You do not need to replace your operating
system with NixOS to use this repository's flake.

The flake supports the following platforms:

- Linux on x86_64 and AArch64;
- macOS on Apple Silicon.

The pinned nixpkgs revision no longer supports Intel macOS (`x86_64-darwin`).
Use the native Cargo workflow on that platform.

On Windows, the Dev Container is the simplest way to use the complete pinned
environment. The alternatives are the native Cargo workflow below or the Linux
flake through WSL2. WSL2 can run either
[NixOS-WSL](https://nix-community.github.io/NixOS-WSL/) or another Linux
distribution with the Nix package manager installed. Native Windows itself is
not one of the systems currently declared by `flake.nix`.

#### Getting Nix

Choose the case that matches your machine:

- **On NixOS, including NixOS-WSL:** Nix is already installed as part of the
  operating system. [NixOS-WSL installation
  instructions](https://nix-community.github.io/NixOS-WSL/install.html) are
  available for users who want to run NixOS directly under WSL2. Make sure the
  modern Nix command and flakes are enabled in your NixOS configuration:

  ```nix
  nix.settings.experimental-features = [ "nix-command" "flakes" ];
  ```

  Apply the configuration with `sudo nixos-rebuild switch`, then continue with
  `nix develop` or the direnv workflow below.

- **On another Linux distribution, macOS, or a non-NixOS WSL2 distribution:**
  install Nix separately as an additional package manager. It works alongside
  tools such as `apt`, `dnf`, `pacman`, or Homebrew and does not replace them.
  Follow the official [Nix download and installation
  instructions](https://nixos.org/download/) for your platform, restart the
  shell if requested, and verify the installation with `nix --version`. The
  official page recommends a multi-user installation when the platform supports
  it.

In either case, Nix reads `flake.nix` and `flake.lock` from this repository to
create the same project-specific toolchain without installing those development
tools globally. The first invocation may take some time because Nix must
download the pinned dependencies; later invocations reuse its local store.

#### Nix Without direnv

Install Nix with flakes enabled, clone the repository, and enter the development
shell manually:

```sh
git clone https://github.com/mveril/RustiQ.git
cd RustiQ
nix develop
```

The default `full` shell provides the Rust toolchain selected by
`rust-toolchain.toml`, the development utilities, and a Python environment
built from the checked-in `uv.lock`. PySCF and the scientific Python
dependencies are available on every platform declared by the flake. Nix places
that environment directly on `PATH`, so no virtual environment needs to be
created or activated:

```sh
python -c "import pyscf; print(pyscf.__version__)"
```

Platform-specific profiling and debugging tools are included where available.
Run the usual Cargo commands inside the shell:

```sh
cargo build
cargo test
cargo run -- run samples/h2/sto-3g/calculation.toml
```

Four shells are available so that contributors only load the tools needed for
their current task:

| Shell        | Contents                                                                 | Command                    |
| ------------ | ------------------------------------------------------------------------ | -------------------------- |
| `mini-rust`  | Rust toolchain and native build dependencies                             | `nix develop .#mini-rust`  |
| `rust`       | Complete Rust development, debugging, and profiling tools without Python | `nix develop .#rust`       |
| `mini-pyscf` | Minimal Rust build environment plus Python and PySCF                     | `nix develop .#mini-pyscf` |
| `full`       | Complete Rust and scientific Python environment                          | `nix develop .#full`       |

Running `nix develop` without a shell name selects `full`.

Leave the environment with `exit` or Ctrl-D. You can also build the default Nix
package without entering the development shell:

```sh
nix build
```

#### Nix With direnv

Install both Nix and `direnv`, enable the direnv hook for your shell, then run:

```sh
git clone https://github.com/mveril/RustiQ.git
cd RustiQ
direnv allow
```

The tracked `.envrc` loads the `full` shell automatically whenever you enter
the repository and unloads it when you leave. To select a lighter shell for one
checkout, create an ignored `.envrc.local`, then allow the updated environment:

```sh
printf '%s\n' 'export RUSTIQ_DEV_SHELL=rust' > .envrc.local
direnv allow
```

Valid values are `mini-rust`, `rust`, `mini-pyscf`, and `full`. The tracked
`.envrc` rejects other values before passing the selection to Nix. `direnv
allow` is deliberately required the first time, and again after either envrc
file changes, so that shell code is not executed without review. Use `direnv
deny` to revoke permission.

Inside the Dev Container, FlakeEnv performs this integration for VS Code. The
tracked `.envrc` remains useful for developers who use direnv in another editor
or terminal.

If `use flake` is unknown, install or configure
[`nix-direnv`](https://github.com/nix-community/nix-direnv), or use `nix develop`
directly. Some direnv/Nix installations already provide this integration.

#### Without Nix Or direnv

Install Git and Rust through [`rustup`](https://rustup.rs/), then use Cargo
directly on Linux, macOS, or Windows:

```sh
git clone https://github.com/mveril/RustiQ.git
cd RustiQ
rustup show
cargo build
cargo test
cargo run -- run samples/h2/sto-3g/calculation.toml
```

`rustup show` causes rustup to notice `rust-toolchain.toml` and install the
requested stable toolchain and components if necessary. This route is enough to
build and run RustiQ, but the extra tools and the PySCF reference environment
from the Nix development shell must be installed separately if you need them.
The repository uses [`uv`](https://docs.astral.sh/uv/) for that environment on
Linux, macOS, and WSL2:

```sh
uv run --locked pytest tools/reference
```

PySCF does not support native Windows; use WSL2 for this optional comparison.

### Installation From The Repository

To install RustiQ directly from the source repository with Cargo, clone the
project and run:

```sh
git clone https://github.com/mveril/RustiQ.git
cd RustiQ
cargo install --path .
```

This installs the `rustiq` binary into Cargo's local binary directory. If you
want a development build instead, use `cargo run` from the repository root.

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

### Machine-readable JSON output

For automation and scientific validation, request the versioned JSON result
contract. JSON mode writes only machine-readable data to standard output; normal
output remains the human-oriented default.

```sh
cargo run -- run samples/h2/sto-3g/calculation.toml --format json
```

Schema version 1 reports the resolved HF method, convergence and final SCF
energies, orthogonalization rank information, and (when requested) MP2 energies.
Floating-point quantities are JSON numbers serialized directly from RustiQ's
`f64` results, without display rounding. For example:

```json
{
  "schema_version": 1,
  "calculation": {
    "hf": {
      "method": "RHF",
      "converged": true,
      "iterations": 2,
      "electronic_energy": -1.831863646477507,
      "nuclear_repulsion_energy": 0.715104339081081,
      "total_energy": -1.116759307396426,
      "delta_energy": 0.0,
      "residual_norm": 0.0,
      "orthogonalization": {
        "ao_basis_dimension": 2,
        "effective_rank": 2,
        "discarded_directions": 0,
        "relative_linear_dependency_threshold": 1e-8
      }
    }
  }
}
```

Future additions may extend this versioned schema; consumers should check
`schema_version` and ignore fields they do not use. Use JSON output for tools;
the default terminal report is intentionally free to evolve for human readers.

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

These commands are identical in the Dev Container, a `nix develop` shell, and a
native Rust installation. The pinned Nix environment additionally provides
tools such as `cargo-nextest`, `cargo-llvm-cov`, `cargo-deny`, `cargo-watch`,
`bacon`, `hyperfine`, `nixd`, `nixfmt`, and Ruff.

Most unit tests are colocated with implementation modules in `src/`. Shared
fixtures live in `tests/data/`, and sample calculation inputs live in
`samples/`.

See also:

- `CONTRIBUTING.md` for contribution guidelines.
- `ROADMAP.md` for project priorities and research-grade requirements.
- `CITATION.cff` for citation metadata.
