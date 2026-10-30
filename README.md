# RustiQ

[![CI](https://github.com/mveril/RustiQ/actions/workflows/ci.yml/badge.svg)](https://github.com/mveril/RustiQ/actions/workflows/ci.yml)

RustiQ is an experimental Rust quantum chemistry application and reusable
`rustiq-chem-core` library. RustiQ-authored source is licensed under MIT OR
Apache-2.0. Executables include LGPL libraries, third-party notices and access
to corresponding sources. See [distribution terms](DISTRIBUTION.md).
It supports RHF, UHF,
MP2, XYZ geometries, Gaussian basis sets, TOML/Nickel inputs, sequential studies,
versioned JSON output, and portable `.rustiq` bundles with reuse of atomic-orbital
electron-repulsion integrals (AO ERIs).

Do not use results for research conclusions without independent validation
against established quantum chemistry packages.

## Purpose

RustiQ explores what modern Rust software engineering can bring to quantum
chemistry through strong typing, memory safety, explicit error handling,
modularity, and maintainability. These engineering goals go hand in hand with
scientific correctness, numerical validation, and performance; RustiQ does not
claim superiority over mature quantum chemistry packages or established
Fortran implementations.

## Quick start

For a prebuilt executable, see [installation](https://mveril.github.io/RustiQ/getting-started/installation.html)
and [the first H₂ calculation](https://mveril.github.io/RustiQ/getting-started/first-calculation.html).
Release archives contain the executable and licenses; obtain basis data and
create calculation inputs as shown in those guides.

To build from source, install Git, Rust through [rustup](https://rustup.rs/), a
C/C++ build toolchain, and CMake; see [manual setup](docs/src/development/manual-setup.md).
Alternatively, enter `nix develop .#rust` after cloning:

```sh
git clone https://github.com/mveril/RustiQ.git
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
