# Command-line reference

Run these commands from any working directory with the installed `rustiq`
binary. From the repository root in a source checkout, replace `rustiq` with
`cargo run --locked --`. `rustiq --help` and `rustiq <command> --help` show the
options supported by the installed build.

## Commands

| Command | Purpose |
| --- | --- |
| `rustiq init <XYZ>` | Create a TOML calculation input that refers to an XYZ geometry. |
| `rustiq run [INPUT]` | Run one TOML or Nickel input. With no path, read TOML from standard input. A Nickel array with multiple calculations runs as a batch. |
| `rustiq basis import <PATH>...` | Import basis JSON files into the local basis store. |
| `rustiq basis list [--online] [--verbose]` | List locally stored bases; online listing is available in builds with the `online` feature. |
| `rustiq basis download <NAME>` | Download a basis in builds with the `online` feature. |
| `rustiq basis remove <NAME>...` / `--all` | Remove selected bases or the local basis store contents. |
| `rustiq cache list [--cache-dir DIR]` | Inspect AO ERI cache entries and their validation status. |
| `rustiq cache remove <NAME-OR-FINGERPRINT> [--cache-dir DIR]` / `--all` | Remove selected or all AO ERI cache entries. |
| `rustiq geometry info [XYZ]` | Print basic geometry information; read XYZ from standard input when no path is given. |
| `rustiq geometry rotate`, `translate`, `center`, `oriente`, `isometry` | Transform a geometry and write the result; each subcommand has its own help. |

The geometry tools and basis storage are utilities around a calculation; they
do not create a `.rustiq` portable artifact. See the
[geometry guide](../user-guide/geometry.md) and [existing workflows](existing-workflows.md)
for examples of geometry, `init`, and basis management commands.

`init` options include `--output` (`calculation.toml` by default), `--force`,
`--basis` (`sto-3g`), `--charge` (zero), `--multiplicity`, `--units`
(`angstrom` by default, or `bohr`), `--hf` (`auto`, `rhf`, or `uhf`), and
`--mp2`. Without an explicit multiplicity, it selects singlet for an even
electron count and doublet for an odd count. This is a starting convention;
choose the physical state appropriate to the molecule. The [existing
workflows](existing-workflows.md) documents path and overwrite behavior.

## `run` options and input selection

The input filename extension selects TOML or Nickel (`.ncl`). TOML describes a
single calculation. Nickel can describe one calculation or an ordered array;
an array of one uses the single-calculation output contract, while a larger
array uses the batch contract. Nickel imports resolve relative to the importing
file. Geometry paths resolve relative to the top-level input file; for TOML
read from standard input, they resolve from the current working directory.

Important options:

| Option | Behavior |
| --- | --- |
| `--format FORMAT` (`text` or `json`) | Select text output (default) or versioned JSON. JSON result data is written to standard output. |
| `--pretty` | Pretty-print JSON, optionally with syntax highlighting. Requires `--format json`. |
| `--color MODE` (`auto`, `always`, or `never`) | Control ANSI colors globally; also accepts `RUSTIQ_COLOR`. Use `--color never` when redirecting pretty JSON. |
| `--auto-download` / `--no-auto-download` | In online builds, allow or disable fetching a missing basis for this run. These override `RUSTIQ_AUTO_DOWNLOAD`; without either flag, `1` or `true` (case-insensitive) enables it and other or missing values disable it. Offline builds always use local bases. |
| `--cache-dir DIR` | Choose the AO ERI cache root for this run. The input must also set `[cache].enabled = true`; this option does not enable caching. |

The cache directory path is resolved from the caller's current working
directory. Pass the same `--cache-dir` to `cache list` or `cache remove` to
manage entries from a run that used a custom cache root.

The CLI reports input, basis, preparation, and single-run calculation errors
as diagnostics. A single HF calculation that reaches its iteration limit
returns its final-iteration result with a successful process status and, in
JSON, `converged: false`. MP2 requires converged HF. For a multi-calculation
Nickel batch, recoverable per-entry errors do not prevent later entries from
running; the JSON document records each outcome, but any error or
non-convergence makes the overall process exit unsuccessfully. Fatal
infrastructure failures stop the batch. JSON output for an unsuccessful batch
is still a valid document. The exact output fields are defined by the
[versioned schemas](index.md#authoritative-references); see the
[JSON guide](../user-guide/json-output.md) for examples and interpretation.
For a single calculation, an input, preparation, or runtime failure is
reported as a diagnostic and does not produce result JSON. A non-converged
single HF run is a result, not a runtime failure.

## Feature availability

| Capability | Available behavior |
| --- | --- |
| TOML | One calculation per input; see the checked-in [H₂ input](../user-guide/configuration.md). |
| Nickel | One calculation or an ordered study/batch; see the checked-in [study example](../user-guide/nickel-studies.md). |
| JSON | Versioned single-calculation and batch results, selected with `run --format json`; schemas are linked below. |
| AO ERI cache | Optional local reuse through `[cache].enabled` and `--cache-dir`; entries can be listed or removed. AO ERI means atomic-orbital electron-repulsion integral. |
| Portable `.rustiq` bundle | V1 format and Rust APIs exist; no create, reuse, or inspect command is exposed by the current CLI. |
| Scientific status | Experimental; selected checks do not establish research-grade correctness. See [scientific scope and limitations](../scientific-scope.md). |

Basis downloads and online basis listing require a build with the `online`
feature. The standard CLI enables it; offline builds provide import, local
listing, and removal. Geometry transformations are CLI utilities, not
calculation input transformations unless their output file is then used as an
input.

From the repository root, these checked-in examples connect the primary
interfaces to executable sample coverage:

```sh
cargo run --locked -- run samples/h2/sto-3g/calculation.toml
cargo run --locked -- run samples/h2/study.ncl --format json
cargo run --locked -- run samples/h2/sto-3g/calculation-cache.toml --cache-dir /tmp/rustiq-cache
cargo run --locked -- cache list --cache-dir /tmp/rustiq-cache
```

The H₂ TOML and Nickel examples are covered by `tests/cli_samples.rs`,
`tests/nickel_cli.rs`, and `tests/json_output_cli.rs`. The cache-enabled input
is `samples/h2/sto-3g/calculation-cache.toml`; its cache behavior is exercised
by the Rust core persistence tests. These commands require STO-3G to be
available locally or basis auto-download to be enabled in an online build.

## Calculation input contracts

TOML and Nickel inputs use the same calculation settings. The embedded Nickel
contracts are authoritative for defaults, allowed fields, types, and
validation; the CLI does not define a separate TOML-only set of defaults.
Consult the [TOML guide](../user-guide/configuration.md) for sections and
checked-in examples, then the [geometry](../user-guide/geometry.md),
[Hartree–Fock](../user-guide/hartree-fock.md), and
[MP2](../user-guide/mp2.md) guides for the corresponding scientific settings.
The Nickel contracts and their validation are linked from the
[reference index](index.md#authoritative-references).

The following settings are accepted by both TOML and Nickel. This is a
navigation aid, not a second schema: exact defaults, value constraints, and
cross-field validation remain defined by the linked Nickel contract.

| Section | Supported settings |
| --- | --- |
| `[molecule]` | `geometry`, `charge`, `multiplicity`, `units` (`Angstrom` or `Bohr`) |
| `[basis]` | `name` |
| `[method.hf]` | `method` (`Auto`, `Rhf`, `Uhf`), `max_iterations`, `convergence_threshold`, `guess`, `diis.enabled`, `diis.max_history`, `orthogonalization.linear_dependency_threshold` |
| `[method.mp2]` | `frozen_orbitals`, `memory_limit` |
| `[integrals]` | `schwarz_threshold` for integral screening |
| `[cache]` | `enabled` for the disposable local AO ERI cache |
| `[output]` | `scf` (`Normal` or `Quiet`) |

The accepted density guesses and their fields are defined in the Nickel
contract; see the HF guide for their use. The cache stores AO ERI data only; it
does not store Hartree–Fock or MP2 results, and a cache hit does not guarantee
that every calculation will run faster.

## Cache and portable bundles

The local AO ERI cache (atomic-orbital electron-repulsion integrals) is a
disposable optimization for deterministic two-electron integrals. It is
disabled by default in the calculation input;
`--cache-dir` only selects where enabled cache entries are stored. Use
`cache list` and `cache remove` to inspect or remove entries. Missing, invalid,
or incompatible entries are recomputed. The local cache is not a deliverable
or a portable calculation file.

Portable `.rustiq` bundles use the V1 ZIP/ZIP64 format described by the
[authoritative persistence specification](https://github.com/mveril/RustiQ/blob/main/docs/persistence-format-v1.md).
The current CLI does not expose `--artifact`, `--reuse`, or `artifact inspect`;
portable bundles are accessed through the Rust persistence APIs. The
[reference index](index.md#authoritative-references) links both the
specification and Rust API documentation. Do not treat cache commands as
portable-bundle operations.

## Output contracts

`run --format json` selects one of two versioned contracts: the single
calculation schema or the batch schema. `schema_version` identifies the
contract; schemas, rather than this manual, define the complete field sets.
The [JSON guide](../user-guide/json-output.md) describes output selection,
standard output behavior, failure cases, and the existing automated checks.
