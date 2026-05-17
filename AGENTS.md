# Repository Guidelines

## Project Structure & Module Organization
RustiQ is a Rust 2021 Cargo binary crate. The executable entry point is `src/main.rs`, with CLI wiring in `src/cli/` using `clap`. Domain modules live under `src/basis/`, `src/molecules/`, `src/hf/`, `src/runfile/`, `src/math_utils.rs`, and `src/eri.rs`. Unit tests are colocated in `#[cfg(test)] mod tests` blocks next to the code they exercise. Shared test helpers are in `src/test_utils.rs`. Example inputs are in `samples/`, and fixture data is in `tests/data/`.

## Build, Test, and Development Commands
- `cargo build`: compile the project in debug mode.
- `cargo run -- run samples/calculation.toml`: run a sample calculation file through the CLI.
- `cargo test`: run the full unit test suite.
- `cargo fmt`: format Rust code with `rustfmt`.
- `cargo clippy --all-targets --all-features`: run lints across the crate before submitting larger changes.

Keep `Cargo.lock` committed because this repository builds an application, not a reusable library.

## Coding Style & Naming Conventions
Follow standard Rust formatting: four-space indentation, `snake_case` for functions, modules, and variables, `PascalCase` for types and enum variants, and `SCREAMING_SNAKE_CASE` for constants. Prefer small modules that mirror the current directory structure, for example `src/basis/gaussian/shell.rs` for Gaussian shell behavior. Use typed errors such as `thiserror` where appropriate instead of stringly typed failures. Keep comments focused on non-obvious math, chemistry assumptions, or CLI behavior.

## Testing Guidelines
Use Rust’s built-in test framework. Place focused unit tests in the same file as the implementation, following the existing `mod tests` pattern. Name tests by the behavior being checked, such as `test_distance_matrix` or `test_boys_function_zero`. Put reusable fixtures in `tests/data/` and small sample run configurations in `samples/`. Run `cargo test` before opening a pull request; add tests when changing numerical routines, parsers, basis-set handling, or SCF behavior.

## Commit & Pull Request Guidelines
The current history only contains an initial commit, so use clear, imperative commit messages going forward, for example `Add XYZ geometry parser` or `Fix SCF convergence threshold`. Pull requests should include a short summary, the commands used for verification, and any relevant input files or numerical output changes. Link related issues when available. For CLI or output formatting changes, include before/after snippets rather than screenshots unless terminal rendering is visually important.

## Agent-Specific Instructions
Avoid broad refactors while addressing targeted issues. Preserve existing sample and fixture files unless the task explicitly requires updating expected behavior. Do not remove user-created local changes; inspect the working tree before large edits.
