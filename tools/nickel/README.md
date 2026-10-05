# Nickel calculation frontend POC (PR #94)

This isolated experiment follows the current `src/runfile` format. It does not
change the CLI or execute scientific calculations. The Rust tests evaluate inputs
with the embedded Nickel library and deserialize the resulting JSON into a DTO.
The example executable also accepts JSON exported by the Nickel CLI. CI installs Nickel CLI 1.17.0; the embedded library is `nickel-lang` 2.2.

This CLI-based flow demonstrates the configuration contract and normalized
shape. The Rust DTO normalizes either input shape to
`ResolvedInput { calculations: Vec<_> }`, and materializes HF defaults even when
the input omits the method section. Those choices follow issue #94. It is not the
planned production integration: RustiQ will use the `nickel-lang` Rust library
to evaluate Nickel configurations in-process.

The pipeline is: TOML or Nickel → `Calculation` contract → injected defaults →
fully evaluated JSON → the typed Rust DTO in `examples/nickel_poc.rs`.
The contract uses [record defaults and custom contracts](https://nickel-lang.org/user-manual/contracts/).
Export forces lazy checks, including fields that a consumer might otherwise never read.

## Run

Enter the repository development shell (`nix develop`) to get Nickel on `PATH`.
From the repository root:

```sh
nickel export --format json tools/nickel/import-toml.ncl
nickel export --format json tools/nickel/single.ncl > /tmp/rustiq-nickel-single.json
cargo run --offline --no-default-features --example nickel_poc < /tmp/rustiq-nickel-single.json
nickel export --format json tools/nickel/multiple.ncl > /tmp/rustiq-nickel-multiple.json
cargo run --offline --no-default-features --example nickel_poc < /tmp/rustiq-nickel-multiple.json
nickel export --format json tools/nickel/variants.ncl
```

`import-toml.ncl` imports the entire real `samples/h2/sto-3g/calculation.toml`
directly, retaining its geometry string. No hand-translated subset is involved.
`single.ncl` creates one calculation. `multiple.ncl` uses `std.array.map` to create
three calculations with different basis names. `variants.ncl` exercises all guess
variants and conditional random/perturbation defaults.

For the real TOML, normalization adds charge `0`, multiplicity `1`, units
`Angstrom`, HF method `Auto`, 100 iterations, convergence threshold `1e-8`,
disabled DIIS with history `6`, orthogonalization threshold `1e-8`, no guess
perturbation, Schwarz threshold `1e-12`, disabled cache, and `Normal` SCF output.
Absent MP2 stays `null`: adding an MP2 section would enable a calculation that
the input did not request. Present MP2 gets zero frozen orbitals and `auto` memory.
An omitted method still resolves to a complete HF configuration; HF is always
present after resolution.

## Verification

```sh
cargo test --offline --no-default-features --example nickel_poc
cargo test --offline --no-default-features --example nickel_poc -- --ignored
cargo test --offline --no-default-features --bin RustiQ nickel_poc_matches_current_runfile_defaults
```

Set `NICKEL_BIN` to an absolute executable path if Nickel is not on `PATH`.
The Rust integration tests run in normal Cargo tests without a Nickel executable.
They use `nickel-lang` to load all four input fixtures, including the real TOML,
force evaluation, deserialize `ResolvedInput`, reject every `.ncl` file in
`invalid/`, and check batch ordering, MP2 defaults, and all guess variants.
Focused tests check defaults, a source-bearing diagnostic, and preservation of
nested data and contract checks through the import workaround. The library is
a dev-dependency for this POC only.

The CLI round-trip test remains ignored in normal Cargo runs and is explicitly
executed by both the Cargo and Nix CI jobs in `.github/workflows/ci.yml`.
It checks all four inputs and every invalid fixture. Both jobs also explicitly
run the Rust integration tests and TOML default parity test.

`rebuild-data.ncl` recursively rebuilds imported records and arrays before
contracts are applied. This avoids Nickel 2.2 debug assertions without modifying
the dependency or disabling checks. See [the reproducer and diagnosis](embedded-panic.md).
The TOML parity test imports every valid current sample through this workaround
and compares the normalized result with the full TOML serialized by the current
Rust frontend, after removing JSON nulls (TOML has no null). This checks default
parity without duplicating expected values in a snapshot.

To inspect a deliberately invalid configuration:

```sh
nickel export --format json tools/nickel/invalid/uniform.ncl
```

Invalid examples cover missing basis, unknown fields, zero multiplicity,
negative convergence threshold, unknown guess, reversed uniform bounds,
invalid DIIS history, wrong field types, zero normal standard deviation, and
empty calculation batches. They must exit unsuccessfully with a Nickel diagnostic.

## Miette diagnostic POC

```sh
cargo run --offline --no-default-features --example nickel_miette_poc -- tools/nickel/invalid/multiplicity.ncl
cargo run --offline --no-default-features --example nickel_miette_poc -- tools/nickel/import-toml.ncl
cargo test --offline --no-default-features --example nickel_miette_poc
```

The first command intentionally exits unsuccessfully and displays the contract
error with miette. The second evaluates the real TOML input and prints JSON.
Without an argument, the example demonstrates the invalid multiplicity input.
No Nickel executable is needed.

The adapter uses `nickel-lang-core`'s `Program`, `IntoDiagnostics`, and source
cache. It copies messages, severity, notes, primary and secondary byte spans,
and exact source contents into owned miette diagnostics. The primary source is
attached to the diagnostic; additional files are related diagnostics with their
own source text. Nickel-generated sources are retained as well. Source text is
captured from Nickel's cache rather than reread from disk after evaluation.
Errors for which Nickel provides no location, such as exporting a function,
retain their message and notes without an invented source span.

For invalid multiplicity, the report includes:

```text
contract broken by the value of `multiplicity`
  calculation.ncl:73     expected type
  invalid/multiplicity.ncl:2     applied to this expression
```

This is an isolated example, not a production frontend change. The stable
`nickel-lang` 2.2 interface offers formatted text and JSON diagnostics, but its
JSON only contains file identifiers and spans, not the corresponding source
contents. A native miette source rendering therefore uses the lower-level
`nickel-lang-core` API here. Both it and `codespan-reporting` are development
dependencies already present transitively; no new package versions are added.
This coupling should be reconsidered when the stable API exposes structured
diagnostics and their source texts.

Cargo and Nix CI explicitly run this example's tests. They cover syntax errors
with UTF-8 offsets, imported contracts spanning multiple files, all invalid POC
fixtures, retained notes, JSON export errors, and all valid input fixtures.

## Boundaries

Geometry paths remain strings relative to the original input's directory; this
POC does not resolve paths relative to exported JSON. It neither opens geometries
nor validates electron counts, basis availability, or scientific method compatibility.
Optional values use JSON nulls; the DTO injects no defaults.
Memory limits are strings here: the production Rust byte-size parser remains the
authority for units and addressable limits. This is not a replacement for all
production validation. Integer limits assume a 64-bit target. Random distributions
with explicit tags require their parameters, as in the existing TOML frontend.
