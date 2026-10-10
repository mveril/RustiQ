# RustiQ

[![CI](https://github.com/mveril/RustiQ/actions/workflows/ci.yml/badge.svg)](https://github.com/mveril/RustiQ/actions/workflows/ci.yml)

RustiQ is an experimental Rust quantum chemistry application and reusable
`rustiq-core` library, licensed under MIT OR Apache-2.0. It supports RHF, UHF,
MP2, XYZ geometries, Gaussian basis sets, TOML/Nickel inputs, sequential studies,
and versioned JSON output.

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

Install Git and Rust through [rustup](https://rustup.rs/), or enter
`nix develop .#rust` after cloning:

```sh
git clone https://github.com/mveril/RustiQ.git
cd RustiQ
cargo build --locked
cargo run --locked -- basis import tests/data/sto-3g.json
cargo run --locked -- run samples/h2/sto-3g/calculation.toml
```

The checked-in basis fixture avoids online basis downloads for this first
calculation. Run these commands from the repository root.

## Documentation

- [RustiQ manual](https://mveril.github.io/RustiQ/): installation, first H₂
  calculation, output interpretation, and development.
- [Book sources](docs/src/SUMMARY.md) for reading before the first deployment.
- [Contributing](CONTRIBUTING.md), [roadmap](ROADMAP.md), and
  [persistence V1 specification](docs/persistence-format-v1.md).
- Generate the Rust API reference with `tools/generate-rustdoc.sh --open`,
  or `.\tools\generate-rustdoc.ps1 -Open` on Windows.

Build with `nix build .#book`, or preview with `mdbook serve docs --open` inside
`nix develop .#rust`. See
[documentation checks](docs/src/development/documentation.md) for coverage.
