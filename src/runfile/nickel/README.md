# Nickel migration schema

PR 3 of #94 makes the embedded Nickel schema authoritative for TOML defaults
and validation. TOML source mapping is kept separate and supplies spans only.
Canonical rendering and `init` still use the legacy frontend until PR 5.

Nickel errors are adapted to miette using original TOML locations when a
contract error identifies an explicit field. Nickel-injected defaults have no
source span. After a failed export, the frontend evaluates independent fields
through Nickel field access and collects their failures as related miette
diagnostics. A failed parent contract is reported once, since its children
cannot be validated until the parent is corrected. Successful inputs keep a
single evaluation pass.
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

The accepted `Random` density guess syntax remains unchanged: its distribution
must be specified explicitly, as before the Nickel migration.
