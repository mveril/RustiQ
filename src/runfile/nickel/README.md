# Nickel migration schema

PR 2 of #94 retains the TOML parser as the production authority. Parsed TOML
crosses `ResolvedInput` before conversion to core configuration. Canonical
rendering and `init` still use the legacy frontend until PR 5.

The embedded Nickel 2.2 evaluator is exercised only in shadow compatibility
tests (`src/runfile/nickel.rs`); normal runs do not evaluate inputs twice.
`calculation.ncl` owns the intended defaults and closed contracts, `resolve.ncl`
normalizes a record or a non-empty array, and `rebuild-data.ncl` is a private
migration workaround. No external Nickel executable is needed.

Nickel 2.2.0 / nickel-lang-core 0.18.0 asserts `value.is_constant()` in
`eval/fixpoint.rs` when a nested deserialized record receives a record contract.
Recursive mapping rebuilds records and arrays into deferred computations before
contract application, avoiding the assertion without patching dependencies or
disabling debug assertions. Imported TOML compatibility tests and nested JSON
regression tests must remain while this workaround is needed. Replace it after
an upstream fix has been verified against those tests. It is not part of the
public input schema. See exploratory POC #96 for the diagnosis.

Memory limits use the existing Rust byte-size parser, shared by TOML and Serde.
Source spans are attached separately after fallible conversion to core types.
An omitted HF section produces fully resolved HF defaults without source spans.

For file inputs, relative geometry paths are resolved from the directory of the
top-level input; absolute paths are retained. For stdin, relative paths are
resolved from the current working directory. CLI cache-directory arguments stay
relative to the caller's working directory. The process directory is never
changed to resolve resources.

Verification:

```sh
cargo test --offline --no-default-features --bin RustiQ runfile::
cargo test --offline --workspace --all-targets --all-features
cargo test --offline --workspace --all-targets --no-default-features
cargo clippy --offline --workspace --all-targets --all-features -- -D warnings
```

One legacy discrepancy found by the additional parity tests is retained:
`[method.hf.guess]` containing only `type = "Random"` is currently rejected by
the TOML parser. The intended Nickel contract supplies the default Uniform
parameters for this input, as validated in POC #96. PR 2 does not make Nickel
authoritative for production TOML or silently broaden its accepted syntax.
