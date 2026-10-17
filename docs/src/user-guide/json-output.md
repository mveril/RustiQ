# JSON output

Examples in this chapter that use `samples/` or `tests/data/` require a cloned
repository and should be run from its root. Release binaries do not include
those files; the [first-calculation guide](../getting-started/first-calculation.md)
shows how to create local inputs and obtain a basis.

Use JSON when another program needs calculation results. `run` writes only
the machine-readable result to standard output. After following the release-
binary path in the [first-calculation guide](../getting-started/first-calculation.md),
use the locally created input:

This writes a JSON file without terminal color:

```sh
rustiq run calculation.toml --format json --color never > result.json
```

For a source checkout, run commands from the repository root and make the
named basis available first. To save machine-readable output, redirect
standard output and disable color explicitly:

```sh
rustiq run samples/h2/sto-3g/calculation.toml --format json
rustiq run samples/h2/sto-3g/calculation.toml --format json --color never > result.json
```

For readable terminal inspection, add `--pretty`:

```sh
rustiq run samples/h2/sto-3g/calculation.toml --format json --pretty
```

`--pretty` requires `--format json`. Compact JSON is unstyled; when saving
pretty output for another tool, use `--color never` to disable syntax colors.

A TOML input produces one calculation result. A one-entry Nickel array also
uses the single-result schema. A multi-entry Nickel study produces a batch
document with one outcome per input, in source order. Each entry can be
`success`, `non_converged`, or `error`. Later calculations continue after a
recoverable per-calculation error, but fatal infrastructure errors stop the
batch. A valid JSON document can accompany a nonzero exit code. A single
HF-only calculation that does not converge still produces the single result
with `converged: false`; its energy and diagnostics describe the final iterate,
and the command returns `3`. If MP2 was requested, non-converged HF prevents
MP2 execution and the run reports a diagnostic with code `3` instead of a
single-result JSON document. Other input, preparation, or runtime errors for a
single calculation return `1` without result JSON.

A completed batch returns `0` when all entries succeed, `4` when some succeed
and others do not, `3` when none succeed and all stop because HF did not converge,
and `1` when none succeed and any entry has another error. Fatal infrastructure
errors always return `1`. See the [CLI exit-code contract](../reference/cli.md)
and [Nickel studies guide](nickel-studies.md) for details.

Results include `schema_version`, which lets consuming software select the
matching contract. The single-calculation contract is
[`calculation-output-v1.schema.json`](https://github.com/mveril/RustiQ/blob/main/schemas/calculation-output-v1.schema.json);
batch output uses
[`batch-output-v1.schema.json`](https://github.com/mveril/RustiQ/blob/main/schemas/batch-output-v1.schema.json).
These versioned schemas are authoritative; this guide does not repeat their
field definitions. See also the [reference index](../reference/index.md).

When `run` uses `--reuse` or `--artifact`, calculation results add an optional
`artifacts` field describing AO ERI selection and its origin. For example, a
bundle hit appears as:

```json
{
  "artifacts": [
    { "name": "ao_eri", "decision": "reused", "origin": "archive", "source_index": 0 }
  ]
}
```

`decision` describes whether the archive supplied a compatible AO ERI;
`origin` identifies the archive, enabled local cache, or fresh computation
that supplied the ERIs actually used. A bundle hit also includes its
zero-based `source_index`. The field is omitted when neither option is used,
and batch results report it on each calculation entry. See the
[portable bundle guide](portable-bundles.md) for the full workflow and the
[CLI reference](../reference/cli.md#cache-and-portable-bundles) for details.

## Nickel batch JSON

The checked-in [Nickel study](nickel-studies.md) runs two calculations. Follow
that guide's basis-set setup first, especially when running offline, then
produce pretty JSON with colors disabled:

```sh
rustiq run samples/h2/study.ncl --format json --pretty --color never
```

For the release-binary path, save the local study as described in the Nickel
guide and use its filename instead, for example
`rustiq run study.ncl --format json --pretty --color never`.

The output follows the versioned batch schema. Each successful calculation
contains a result following the single-calculation schema linked above. Inspect
the `status` of every calculation; a batch can include non-converged or failed
entries even when it produces valid JSON. `--color never` prevents ANSI escape
sequences in redirected output.

Energies are in Hartree. JSON preserves full floating-point values for
comparison and downstream processing, while terminal tables round energies
for readability. Do not compare a rounded terminal value against a tight
numerical tolerance. Existing JSON CLI tests validate both schemas and
full-precision H₂ output in
[`tests/json_output_cli.rs`](https://github.com/mveril/RustiQ/blob/main/tests/json_output_cli.rs).
The checked-in reference tests compare JSON values against PySCF 2.14.0 for
selected systems; see [scientific scope](../scientific-scope.md#reference-comparisons).
For terminology used in result fields, see the [glossary](../glossary.md).
