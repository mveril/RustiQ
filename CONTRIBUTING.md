# Contributing

RustiQ is an experimental Rust quantum chemistry prototype. Contributions should
keep the codebase scientifically honest, testable, and easy to inspect.

## Development Setup

Install stable Rust and Cargo, then run:

```sh
cargo build
cargo test
```

## Checks Before Opening A Pull Request

Run:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo test --all-targets --no-default-features
```

The pull-request workflow also evaluates every system declared by the Nix flake,
builds the native Nix package, and runs a PySCF smoke comparison. A scheduled
and manually dispatchable `Full Nix checks` workflow builds every native flake
check and runs the complete PySCF reference set. Run the same checks locally
with:

```sh
nix flake check --all-systems --no-build
nix flake check
nix run .#pyscf-check
```

If a change affects numerical behavior, include the affected input files,
reference values, and tolerance rationale.

## Numerical Changes

For changes to integrals, SCF, MP2, basis handling, or geometry parsing:

- add or update tests close to the implementation;
- compare against an established package when possible;
- state whether the change affects total energy, electronic energy, correlation
  energy, convergence behavior, or only reporting;
- avoid loosening tolerances without explaining why.

## Documentation

Document scientific conventions when they matter: units, normalization,
spin assumptions, integral ordering, and energy definitions.

## Licensing

Unless explicitly stated otherwise, contributions are accepted under the same
dual license as the repository: MIT OR Apache-2.0.
