# Embedded Nickel nested-data merge panic

## Scope

Observed with `nickel-lang` 2.2.0 and its `nickel-lang-core` 0.18.0 dependency
in a debug build. Importing TOML alone succeeds. Merging a nonempty record
with imported nested data, including applying a record contract, panics.
The same failure occurs with `std.deserialize` JSON data. This does not depend
on RustiQ defaults, scientific calculations, or JSON-to-DTO deserialization.

## Minimal reproducer

This Rust program only needs `nickel-lang = "=2.2.0"` as a dependency:

```rust
fn main() {
    let path = std::env::temp_dir().join("nickel-nested-data.toml");
    std::fs::write(&path, "[nested]\nx = 1\n").unwrap();
    // Escape the path as a Nickel string, including Windows path separators.
    let path = path.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"");
    let source = format!(
        "(import \"{path}\") | {{ nested | {{ x | Number }} }}"
    );
    let mut context = nickel_lang::Context::new();
    context.eval_deep_for_export(&source).unwrap();
}
```

The failing assertion is `value.is_constant()` in
`nickel-lang-core/src/eval/fixpoint.rs:55`, inside `rec_env`.

Control cases:

| Expression | Original dependency | Diagnostic patch |
| --- | --- | --- |
| Import nested TOML alone | Success | Success |
| Import nested TOML merged with `{}` | Success | Success |
| Import nested TOML merged with `{ other = 2 }` | Panic | Success |
| Import nested TOML with a nested record contract | Panic | Success |
| Same contract on a native Nickel nested record | Success | Success |
| Import nested TOML with a default on the contracted field | Panic | Success |
| Same contract on nested JSON from `std.deserialize` | Panic | Success |
| Full `import-toml.ncl` POC input | Panic | Success |

## Cause

1. `serialize::toml_deser` constructs ordinary `NickelValue` records containing
   other ordinary records. These are already data values, rather than the
   deferred expressions used for native Nickel record fields.
2. Applying a record contract uses `VirtualMachine::merge`. When a field has
   a value on the data side and only a contract on the other side,
   `merge_fields` keeps that value through `revert_closurize`.
3. `RevertClosurize for NickelValue` explicitly permits arbitrary deserialized
   data to remain unwrapped. Its implementation returns such data unchanged.
4. Merge constructs a recursive record marked `closurized = true`, so the
   evaluator skips preparing its fields again.
5. `fixpoint::rec_env` nevertheless asserts that every field without a thunk
   is an atomic constant. A nonempty ordinary record is not an atomic constant
   according to `NickelValue::is_constant`, so the assertion fails. The same
   outdated assumption is asserted again in `fixpoint::patch_value`.

An empty merge is special-cased as the identity, which explains why merging
with `{}` does not reproduce the panic.

## Diagnostic verification

In a separate copy of the dependency under `/tmp`, both assertions were changed
only to also accept ordinary record and array values. With debug assertions
still enabled, all cases above succeeded, including the original full POC
input. No repository dependency, lockfile, or evaluator implementation was
changed for this experiment.

This isolates the failure to the two assertions, which do not account for
already evaluated containers allowed by the compact value representation.
The diagnostic change is not a generally validated upstream fix. In particular,
this investigation does not establish that every possible unwrapped container
is safe or that recursive merge semantics are correct for all programs.

## Workaround without a dependency patch

Recursively map imported data before applying record contracts:

```nickel
let Rebuild = import "rebuild-data.ncl" in
let ResolveInput = import "resolve.ncl" in
ResolveInput (Rebuild (import "../../samples/h2/sto-3g/calculation.toml"))
```

The map introduces deferred field computations accepted by recursive merge.
Mapping only the outer record is insufficient for nested data. The recursive
version was verified in debug with the unmodified dependency on the full POC
TOML input, a deeper nested TOML record, and an array of nested JSON records.
A nested JSON field with an incorrect type still produced a contract diagnostic
without panicking. This transformation is intended for imported pure data.

The POC now uses `rebuild-data.ncl` in `import-toml.ncl` and the Rust TOML default
parity test. Cargo tests cover the workaround with nested records inside an
array, unchanged values, incorrect field types, and unknown fields. All four
POC input fixtures, including TOML, are evaluated by the unmodified Rust library.
