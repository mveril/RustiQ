# JSON output

Use JSON when another program needs calculation results. `run` writes only
the machine-readable result to standard output:

```sh
cargo run --locked -- run samples/h2/sto-3g/calculation.toml --format json
```

Run commands from the repository root and make the named basis available
first. To save machine-readable output, redirect standard output and disable
color explicitly:

```sh
cargo run --locked -- run samples/h2/sto-3g/calculation.toml --format json --color never > result.json
```

For readable terminal inspection, add `--pretty`:

```sh
cargo run --locked -- run samples/h2/sto-3g/calculation.toml --format json --pretty
```

`--pretty` requires `--format json`. Compact JSON is unstyled; when saving
pretty output for another tool, use `--color never` to disable syntax colors.

A TOML input produces one calculation result. A one-entry Nickel array also
uses the single-result schema. A multi-entry Nickel study produces a batch
document with one outcome per input, in source order. Each entry can be
`success`, `non_converged`, or `error`. Later calculations continue after a
recoverable per-calculation error, but fatal infrastructure errors stop the
batch. Any recorded error or nonconvergence makes the overall command exit
unsuccessfully. A valid JSON document can therefore accompany a failing exit
status. A single calculation that does not converge still produces the single
result document with `converged: false`; its energy and diagnostics describe
the final iterate, and the command exits successfully. Input, preparation, or
runtime errors for a single calculation are reported as CLI diagnostics, so
there is no single-result JSON document for that failed run. See the
[Nickel studies guide](nickel-studies.md) for batch outcomes.

Results include `schema_version`, which lets consuming software select the
matching contract. The single-calculation contract is
[`calculation-output-v1.schema.json`](https://github.com/mveril/RustiQ/blob/main/schemas/calculation-output-v1.schema.json);
batch output uses
[`batch-output-v1.schema.json`](https://github.com/mveril/RustiQ/blob/main/schemas/batch-output-v1.schema.json).
These versioned schemas are authoritative; this guide does not repeat their
field definitions. See also the [reference index](../reference/index.md).

Energies are in Hartree. JSON preserves full floating-point values for
comparison and downstream processing, while terminal tables round energies
for readability. Do not compare a rounded terminal value against a tight
numerical tolerance. Existing JSON CLI tests validate both schemas and
full-precision H₂ output in
[`tests/json_output_cli.rs`](https://github.com/mveril/RustiQ/blob/main/tests/json_output_cli.rs).
The checked-in reference tests compare JSON values against PySCF 2.14.0 for
selected systems; see [scientific scope](../scientific-scope.md#reference-comparisons).
For terminology used in result fields, see the [glossary](../glossary.md).
