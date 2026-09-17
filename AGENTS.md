# Repository Guidelines

## Project Structure & Module Organization

RustiQ is a Rust 2021 Cargo workspace with two crates. The root `RustiQ` package is the CLI binary: `src/main.rs` is its entry point, `src/cli/` handles commands and reports, and `src/runfile/` parses TOML, reports input errors, and converts runfile options to scientific configuration. The reusable `rustiq-core` library lives in `crates/rustiq-core/`; its `src/config/` and `src/calculation/` modules define and run calculations, while `src/molecules/`, `src/basis/`, `src/eri/`, `src/hf/`, and `src/mp2/` contain the scientific implementation. Keep runfile parsing and terminal presentation in the CLI crate. Example inputs are in `samples/`, CLI fixtures are in `tests/data/`, and core fixtures are in `crates/rustiq-core/tests/data/`.

## Build, Test, and Development Commands

- `cargo build --workspace`: compile both crates in debug mode.
- `cargo run -- run samples/h2/sto-3g/calculation.toml`: run a sample calculation through the CLI.
- `cargo fmt --all -- --check`: check Rust formatting without changing files.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: run the CI lint check.
- `cargo test --workspace --all-targets --all-features`: run tests with all features enabled.
- `cargo test --workspace --all-targets --no-default-features`: check the offline configuration.

Keep `Cargo.lock` committed because the workspace includes an application. The CLI enables the `online` feature by default; `rustiq-core` has no default features and can be checked independently with `cargo test -p rustiq-core --no-default-features`.

## Coding Style & Naming Conventions

Follow standard Rust formatting: four-space indentation, `snake_case` for functions, modules, and variables, `PascalCase` for types and enum variants, and `SCREAMING_SNAKE_CASE` for constants. Prefer small modules that mirror the current directory structure, for example `crates/rustiq-core/src/basis/gaussian/shell.rs` for Gaussian shell behavior. Use typed errors such as `thiserror` where appropriate instead of stringly typed failures. Keep comments focused on non-obvious math, chemistry assumptions, or CLI behavior.

## Testing Guidelines

Use Rust's built-in test framework. Place focused unit tests beside the implementation in `#[cfg(test)] mod tests` blocks; shared core test helpers are in `crates/rustiq-core/src/test_utils.rs`. Integration tests for the CLI are in root `tests/`, and core API tests are in `crates/rustiq-core/tests/`. Put reusable fixtures in the corresponding crate's `tests/data/` directory and small run configurations in `samples/`. The CLI's versioned JSON output schema is in `schemas/calculation-output-v1.schema.json` and is checked by `tests/json_output_cli.rs`. For changes to integrals, SCF, MP2, basis handling, or geometry parsing, add focused tests and compare numerical results with an established package when possible; explain affected energies and tolerance changes. PySCF reference comparisons live in `tools/reference/` and run separately from Cargo tests with `uv run --locked pytest tools/reference`.

## Commit & Pull Request Guidelines

Name every task branch using the `type/name` format, with a lowercase type and a lowercase, hyphen-separated English description. Choose a type that describes the work, such as `feature/*`, `fix/*`, `chore/*`, `docs/*`, `test/*`, `refactor/*`, `perf/*`, or `ci/*`. These are examples, not an exhaustive list; other appropriate types are allowed. For example: `feature/add-xyz-parser`, `fix/scf-convergence`, or `chore/add-mp2-pyscf-reference-cases`. Do not create task branches without a type prefix.

Use clear, imperative commit messages, for example `Add XYZ geometry parser` or `Fix SCF convergence threshold`. Pull requests should include a short summary, the commands used for verification, and any relevant input files or numerical output changes. Link related issues when available. For CLI or output formatting changes, include before/after snippets rather than screenshots unless terminal rendering is visually important.

## Agent-Specific Instructions

Use English exclusively for all repository and GitHub content, including code comments, documentation, commit messages, branch names, pull request titles and descriptions, and review comments.

Avoid broad refactors while addressing targeted issues. Preserve existing sample and fixture files unless the task explicitly requires updating expected behavior. Do not remove user-created local changes; inspect the working tree before large edits.
