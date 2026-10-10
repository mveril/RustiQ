# Nickel studies

Nickel is a configuration language that RustiQ embeds. A `.ncl` file can
import other Nickel files and compute a list of related calculation records.
The checked-in single-calculation `.ncl` input is a small starting point:

```nickel
{{#include ../../../samples/h2/sto-3g/calculation.ncl}}
```

Make sure STO-3G is in the local basis store, then run it with the same command
as TOML:

```sh
cargo run --locked -- basis import tests/data/sto-3g.json
cargo run --locked -- run samples/h2/sto-3g/calculation.ncl
```

The checked-in H₂ study maps over two basis names to produce two inputs:

```nickel
{{#include ../../../samples/h2/study.ncl}}
```

This study is sequential. Run it from the repository root after making both
bases available:

```sh
cargo run --locked -- basis import tests/data/sto-3g.json
cargo run --locked -- basis import tests/data/reference/RustiQ/basis_sets/6-31g.json
cargo run --locked -- run samples/h2/study.ncl
```

When both calculations succeed, selected output excerpts are:

```text
Calculation 1/2
Calculation 2/2
2 succeeded, 0 non-converged, 0 failed
```

The example runs two calculations sequentially, using STO-3G and 6-31G. Both
must succeed for this success summary to appear. These are selected lines, not
the complete terminal output.

The Nickel file uses `molecule.geometry`, `basis.name`, and an HF DIIS setting
to create one calculation for each list item. Geometry paths resolve relative
to the top-level `.ncl` file; Nickel imports resolve relative to the file that
imports them. No external Nickel executable is needed.

TOML inputs describe one calculation. A `.ncl` file with one calculation
behaves as a single calculation; an array with multiple entries produces a
batch. This is useful for studies that vary inputs while keeping the remaining
settings shared. Nickel is validated using the same authoritative input
contracts as TOML; it does not define a second scientific configuration
schema. The project integration test constructs a native import fixture and
checks resolution from a different working directory; see
[`tests/nickel_cli.rs`](https://github.com/mveril/RustiQ/blob/main/tests/nickel_cli.rs#L62).
See [configuration](configuration.md) and the
[Nickel contract source](../reference/index.md).

RustiQ validates all study entries before it starts running them. Calculations
then run in source order. A calculation that has a recoverable runtime error
or does not converge is recorded for that entry, and later entries are still
attempted. Fatal infrastructure errors stop the batch.
The batch returns an unsuccessful process status if any entry fails or does
not converge, while retaining per-calculation results. A configuration error
prevents execution of the batch.

The integration coverage is in
[`tests/nickel_cli.rs`](https://github.com/mveril/RustiQ/blob/main/tests/nickel_cli.rs)
and [`tests/json_output_cli.rs`](https://github.com/mveril/RustiQ/blob/main/tests/json_output_cli.rs).
For structured outcomes, see [JSON output](json-output.md).
See the [glossary](../glossary.md) for Nickel and calculation terminology.
