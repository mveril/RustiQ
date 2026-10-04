# Nickel calculation frontend POC (PR #94)

This isolated experiment follows the current `src/runfile` format. It does not
change the CLI or execute scientific calculations. It uses the Nickel CLI only
to evaluate the examples and export JSON; the Rust example deserializes that JSON
into a DTO. The CLI is Nickel 1.18.0; the embedded library is `nickel-lang` 2.2.

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
cargo test --offline --no-default-features --bin RustiQ nickel_poc_matches_current_runfile_defaults -- --ignored
```

Set `NICKEL_BIN` to an absolute executable path if Nickel is not on `PATH`.
The POC tests are explicitly invoked because normal Cargo builds do not require
Nickel.
The CLI test verifies normalized JSON-to-DTO round trips and rejects every file
in `invalid/`. A separate in-process test uses the `nickel-lang` Rust library to
load the same Nickel configuration and contract, force evaluation, decode
`ResolvedInput`, and check a source-bearing contract diagnostic. The library is
a dev-dependency for this POC only. The parity test imports every valid current
sample in Nickel and compares the normalized result with the full TOML serialized
by the current Rust frontend, after removing JSON nulls (TOML has no null). This
checks default parity without duplicating expected values in a snapshot.

To inspect a deliberately invalid configuration:

```sh
nickel export --format json tools/nickel/invalid/uniform.ncl
```

Invalid examples cover missing basis, unknown fields, zero multiplicity,
negative convergence threshold, unknown guess, reversed uniform bounds,
invalid DIIS history, wrong field types, zero normal standard deviation, and
empty calculation batches. They must exit unsuccessfully with a Nickel diagnostic.

## Boundaries

Geometry paths remain strings relative to the original input's directory; this
POC does not resolve paths relative to exported JSON. It neither opens geometries
nor validates electron counts, basis availability, or scientific method compatibility.
Optional values use JSON nulls; the DTO injects no defaults.
Memory limits are strings here: the production Rust byte-size parser remains the
authority for units and addressable limits. This is not a replacement for all
production validation. Integer limits assume a 64-bit target. Random distributions
with explicit tags require their parameters, as in the existing TOML frontend.
