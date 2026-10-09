# Contributing

RustiQ is an experimental Rust quantum chemistry prototype. Contributions should
keep the codebase scientifically honest, testable, and easy to inspect.

## Development Setup

The recommended VS Code setup is the repository Dev Container. Install Docker
Desktop, VS Code, and the Dev Containers extension, open the repository, then
run **Dev Containers: Reopen in Container**. FlakeEnv loads the pinned Nix
development shell for terminals, tasks, debuggers, and language servers.

Alternatively, enter `nix develop` or install stable Rust and Cargo locally.
Then run:

```sh
cargo build --workspace
cargo test --workspace
```

## Branch Naming

Name every task branch using the `type/name` format, with a lowercase type and
a lowercase, hyphen-separated English description. Choose a type that describes
the work, such as `feature/*`, `fix/*`, `chore/*`, `docs/*`, `test/*`,
`refactor/*`, `perf/*`, or `ci/*`. These are examples, not an exhaustive list;
other appropriate types are allowed.

Examples include `feature/add-xyz-parser`, `fix/scf-convergence`, and
`chore/add-mp2-pyscf-reference-cases`. Do not create task branches without a type
prefix.

## Checks Before Opening A Pull Request

Run:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo test --workspace --all-targets --all-features
cargo test --workspace --all-targets --no-default-features
uv lock --check
```

The pull-request workflow also evaluates every system declared by the Nix flake,
builds the native Nix package, and runs a PySCF smoke comparison. A scheduled
and manually dispatchable `Full Nix checks` workflow builds every native flake
check and runs the complete PySCF reference set. Run the same checks locally
with:

```sh
nix flake check --all-systems --no-build
nix flake check
nix fmt -- --fail-on-change # Nix, Rust, Python, TOML, YAML, JSON, and Markdown
nix run .#pyscf-check
```

Outside Nix, run the same reference tooling with the locked Python environment:

```sh
uv run --locked python tools/reference/compare_pyscf.py
```

If a change affects numerical behavior, include the affected input files,
reference values, and tolerance rationale.

## Rust Lint Policy

Both crates inherit the workspace lint policy from the root Cargo manifest.
Clippy's `all` and `pedantic` groups are denied, together with `unwrap_used`,
`dbg_macro`, `todo`, `unimplemented`, `panic`, `panic_in_result_fn`,
`let_underscore_must_use`, `exit`, `lossy_float_literal`, and
`allow_attributes_without_reason`.
Unsafe code is forbidden. CI checks every target with all features and without
default features; it also rejects any remaining compiler warnings.

Use typed errors for fallible input and operations. Production code must not
panic on fallible external input; a `Result`-returning function must report such
failures through its error. Panics and `expect` are reserved for established
internal invariants and must be locally justified. Tests may panic intentionally
and use `unwrap`; benchmarks follow the production rules. Do not silently
discard a fallible result.

Keep test assertions idiomatic: use `assert!(value.is_empty())` or
`assert!(!value.is_empty())` when testing emptiness. Test modules permit
`assert_is_empty` rather than requiring comparisons with typed empty collections.
These local exceptions tolerate the lint being unavailable on Clippy 1.98.
Avoid explicit type arguments when inference already makes the code clear.

Keep exceptions on the smallest practical item and specify an English `reason`.
Scientific notation, intentional floating-point rounding, exact endpoint
comparisons, shared interface contracts, and cohesive calculation/reporting
routines can justify an exception. Do not suppress all of `pedantic`, disable
integer-conversion checks globally, or rewrite numerical formulas merely to
silence a style lint. Document public errors and panics, and use `#[must_use]`
when ignoring a returned value is likely to be a mistake.

### Domain-specific lint choices

The crate roots add checks specific to their responsibilities:

- `rustiq-core` denies `host_endian_bytes` to keep persisted bytes portable and
  `redundant_clone` where ownership makes a clone unnecessary. It also denies
  `imprecise_flops` to catch avoidable precision loss, such as `exp(x) - 1`
  instead of `exp_m1(x)`, and denies `print_stdout`
  and `print_stderr` so library calculations cannot add terminal output that
  interferes with CLI reports or versioned JSON output.
- The CLI denies `string_slice` to require review of byte-indexed UTF-8 text
  operations and `path_buf_push_overwrite` to catch absolute paths accidentally
  replacing a previously constructed path.
- Both crates deny `lossy_float_literal` for integer-valued float literals that
  cannot be represented exactly, and `exit` to keep explicit process termination
  out of application helpers and library functions.

The existing `all` and `pedantic` groups also cover `approx_constant`,
`excessive_precision`, `float_equality_without_abs`, `float_cmp`, and potentially
lossy integer/float casts. Intentional exact comparisons and scientific reference
constants retain local, documented exceptions.

These choices follow the [official Clippy lint catalogue](https://rust-lang.github.io/rust-clippy/stable/index.html).
Do not enable the entire `restriction` or `nursery` group. In particular,
`float_arithmetic` would prohibit the core's purpose, and `suboptimal_flops`
proposes transformations such as fused multiply-add that may change rounding.
Use the latter as an explicit numerical optimization audit, followed by focused
scientific tests and reference comparisons:

```bash
cargo clippy -p rustiq-core --all-targets --all-features -- -W clippy::suboptimal_flops
```

## Numerical Changes

For changes to integrals, SCF, MP2, basis handling, or geometry parsing:

- add or update tests close to the implementation;
- compare against an established package when possible;
- state whether the change affects total energy, electronic energy, correlation
  energy, convergence behavior, or only reporting;
- avoid loosening tolerances without explaining why.

## Documentation

The [RustiQ manual](https://mveril.github.io/RustiQ/) is built from `docs/src/`.
The Nix `rust` and `full` development shells provide mdBook. Preview changes with
`mdbook serve docs --open`, and run `nix build .#book` to build the manual,
validate Rust snippets, and check local links. See
[documentation checks](docs/src/development/documentation.md) for coverage and
[development environments](docs/src/development/environments.md) for setup.

Document scientific conventions when they matter: units, normalization,
spin assumptions, integral ordering, and energy definitions.

## Licensing

Unless explicitly stated otherwise, contributions are accepted under the same
dual license as the repository: MIT OR Apache-2.0.
