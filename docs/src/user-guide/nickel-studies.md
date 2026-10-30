# Nickel studies

Examples in this chapter that use `samples/` or `tests/data/` require a cloned
repository and should be run from its root. Release binaries do not include
those files; the [first-calculation guide](../getting-started/first-calculation.md)
shows how to create local inputs and obtain a basis.

For a single calculation, [TOML](configuration.md) is sufficient. **Nickel is an
optional interface for parameterized studies** that vary basis sets or numerical
settings without duplicating input files. You do not need Nickel to run TOML
calculations.

Nickel is a configuration language that RustiQ embeds. A `.ncl` file can
import other Nickel files and compute a list of related calculation records.
The checked-in single-calculation `.ncl` input is a small starting point:

```nickel
{{#include ../../../samples/h2/sto-3g/calculation.ncl}}
```

For the release binary, copy the H₂ geometry from the study section below to
`molecule.xyz` in your working directory. Create an `inputs` directory with
`mkdir inputs` and save this input as `inputs/calculation.ncl`. Its
`../molecule.xyz` path resolves without editing. Download STO-3G with
`rustiq basis download sto-3g`, or import an independently obtained local
JSON file with `rustiq basis import /path/to/sto-3g.json` when offline. Then
run `rustiq run inputs/calculation.ncl`.

For a source checkout, make sure STO-3G is in the local basis store, then run
this checked-in input:

```sh
rustiq basis import tests/data/sto-3g.json
rustiq run samples/h2/sto-3g/calculation.ncl
```

The checked-in H₂ study maps over two basis names to produce two inputs:

```nickel
{{#include ../../../samples/h2/study.ncl}}
```

To run this study with only the release binary, save the H₂ geometry below as
`molecule.xyz` and save the study above as `study.ncl` in the same working
directory. The study's `molecule.geometry = "molecule.xyz"` path is relative
to that file. Download both required bases, then run the local study:

```text
{{#include ../../../samples/h2/molecule.xyz}}
```

```sh
rustiq basis download sto-3g
rustiq basis download 6-31g
rustiq run study.ncl
```

This study is sequential. For a source checkout, run it from the repository
root after making both bases available:

```sh
rustiq basis import tests/data/sto-3g.json
rustiq basis import tests/data/reference/RustiQ/basis_sets/6-31g.json
rustiq run samples/h2/study.ncl
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
schema. See [calculation inputs and execution](calculation-inputs.md) for the
shared input flow. The project integration test constructs a native import
fixture and checks resolution from a different working directory; see
[`tests/nickel_cli.rs`](https://github.com/mveril/RustiQ/blob/main/tests/nickel_cli.rs#L62).
See [configuration](configuration.md) and the
[Nickel contract source](../reference/index.md).

RustiQ validates all study entries before it starts running them. Calculations
then run in source order. A calculation that has a recoverable runtime error
or does not converge is recorded for that entry, and later entries are still
attempted. Fatal infrastructure errors stop the batch.
The batch returns an unsuccessful process status if any entry fails or does
not converge, while retaining per-calculation results: `4` for partial success,
`3` when every entry stops because HF did not converge, or `1` when no entry
succeeds and at least one has another error. Fatal infrastructure errors return
`1` regardless of earlier successes. A configuration error prevents execution
of the batch and returns `1`. See the [CLI exit-code contract](../reference/cli.md).

The integration coverage is in
[`tests/nickel_cli.rs`](https://github.com/mveril/RustiQ/blob/main/tests/nickel_cli.rs)
and [`tests/json_output_cli.rs`](https://github.com/mveril/RustiQ/blob/main/tests/json_output_cli.rs).
For structured outcomes, see [JSON output](json-output.md).
To save a study's calculation entries and reusable AO ERIs in one portable
bundle, see [portable calculation bundles](portable-bundles.md).
See the [glossary](../glossary.md) for Nickel and calculation terminology.
