# RustiQ

[![CI](https://github.com/mveril/RustiQ/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/mveril/RustiQ/actions/workflows/ci.yml?query=branch%3Amain)
[![GitHub release](https://img.shields.io/github/v/release/mveril/RustiQ?include_prereleases)](https://github.com/mveril/RustiQ/releases)
[![crates.io](https://img.shields.io/crates/v/rustiq-chem)](https://crates.io/crates/rustiq-chem)
[![Manual](https://img.shields.io/badge/docs-manual-blue)](https://mveril.github.io/RustiQ/)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](DISTRIBUTION.md)

RustiQ is an experimental quantum chemistry program for fixed-geometry
calculations with Gaussian basis sets. It supports restricted Hartree–Fock
(RHF) for closed-shell molecules, unrestricted Hartree–Fock (UHF) for
open-shell systems, and MP2 correlation energies from either reference.

Supply an XYZ geometry and a TOML or Nickel calculation input. Use the
terminal report or versioned JSON for numerical results. A portable
`.rustiq` bundle saves the resolved inputs, converged HF orbitals, and
available AO integrals. You can reuse the saved reference for MP2 without a
second SCF run or export the orbitals in Molden format.

The [water, hydroxyl radical, and benzene tutorials](docs/src/user-guide/examples.md) include the inputs you need to run MP2, inspect an open-shell solution, and plot a benzene orbital using an installed RustiQ binary.
The [PySCF reference suite](tools/reference/README.md) documents the comparisons
and their tolerances. Check results independently before drawing research
conclusions. HF omits electron correlation, and MP2 can be unreliable for
strongly correlated systems. See the [scientific scope](docs/src/scientific-scope.md).

## Quick start

For a prebuilt executable, see [installation](https://mveril.github.io/RustiQ/getting-started/installation.html)
and [the first H₂ calculation](https://mveril.github.io/RustiQ/getting-started/first-calculation.html).
Release archives contain the executable and licenses; obtain basis data and
create calculation inputs as shown in those guides.

To install beta.1 from crates.io, use
`cargo install --locked rustiq-chem --version '=0.1.0-beta.1'`, or install the
tagged flake’s default package with
`nix profile install 'github:mveril/RustiQ/v0.1.0-beta.1'`.
See the installation guide for prerequisites and supported Nix platforms.

To build from source, install Git, Rust through [rustup](https://rustup.rs/), a
C/C++ build toolchain, and CMake; see [manual setup](docs/src/development/manual-setup.md).
Alternatively, enter `nix develop .#rust` after cloning:

```sh
git clone --branch develop --single-branch https://github.com/mveril/RustiQ.git
cd RustiQ
cargo build --locked
cargo run --locked -- basis import tests/data/sto-3g.json
cargo run --locked -- run samples/h2/sto-3g/calculation.toml
```

The checked-in basis fixture avoids online basis downloads for this first
calculation. Run these commands from the repository root.

The product is called **RustiQ**; its Cargo packages are `rustiq-chem`
(CLI) and `rustiq-chem-core` (library). The executable is `rustiq`
(`rustiq.exe` on Windows), and the Rust library import remains `rustiq_core`.
To install from this checkout: `cargo install --locked --path .`.

## Documentation

- [RustiQ manual](https://mveril.github.io/RustiQ/): installation, first H₂
  calculation, output interpretation, and development.
- [Book sources](docs/src/SUMMARY.md) for reading the manual from a checkout.
- [Portable bundles](docs/src/user-guide/portable-bundles.md): save, rerun,
  inspect, verify, and reuse calculation snapshots and compatible AO ERIs.
- [Scientific scope](docs/src/scientific-scope.md) and
  [reference comparisons](tools/reference/README.md): validated cases,
  tolerances, limitations, and reproducible reports.
- [Contributing](CONTRIBUTING.md), [roadmap](ROADMAP.md), and
  [persistence V1 specification](docs/persistence-format-v1.md).
- Generate the Rust API reference with `tools/generate-rustdoc.sh --open`,
  or `.\tools\generate-rustdoc.ps1 -Open` on Windows.

Build the offline manual with `nix build .#book`, or use
`python tools/build-manual.py` with mdBook and mdbook-mermaid installed.
`rustiq docs` opens the bundled manual in release archives and Nix packages;
Cargo-only installations open the online manual. Use `rustiq docs --web` to
force the online version, or `--print-path` for a path/URL without a browser.
See [documentation checks](docs/src/development/documentation.md) for previewing
and verification. PDF export is planned for later.
