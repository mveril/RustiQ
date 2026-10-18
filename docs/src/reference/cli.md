# Command-line reference

Run these commands from any working directory with the installed `rustiq`
binary. In a source checkout without an installed executable, replace the
`rustiq` command prefix with `cargo run --locked --`.
`rustiq --help` and `rustiq <command> --help` show the
options supported by the installed build.

## Commands

| Command | Purpose |
| --- | --- |
| `rustiq init <XYZ>` | Create a TOML calculation input that refers to an XYZ geometry. |
| `rustiq run [INPUT]` | Run TOML or Nickel input, or rerun calculations from a `.rustiq` bundle. With no path, read TOML from standard input. Multi-entry inputs run as batches. |
| `rustiq basis import <PATH>...` | Import basis JSON files into the local basis store. |
| `rustiq basis list [--online] [--verbose]` | List locally stored bases; online listing is available in builds with the `online` feature. |
| `rustiq basis download <NAME>` | Download a basis in builds with the `online` feature. |
| `rustiq basis remove <NAME>...` / `--all` | Remove selected bases or the local basis store contents. |
| `rustiq cache list [--cache-dir DIR]` | Inspect AO ERI cache entries and their validation status. |
| `rustiq cache remove <NAME-OR-FINGERPRINT> [--cache-dir DIR]` / `--all` | Remove selected or all AO ERI cache entries. |
| `rustiq artifact inspect <PATH> [--format FORMAT]` | Inspect portable `.rustiq` calculation snapshots and artifact metadata. |
| `rustiq artifact verify <PATH>` | Check hashes and decode supported numerical payloads in a portable `.rustiq` bundle. |
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

The input filename extension selects TOML, Nickel (`.ncl`), or a portable
`.rustiq` bundle. TOML describes a single calculation. Nickel can describe one
calculation or an ordered array; an array of one uses the single-calculation
output contract, while a larger array uses the batch contract. A `.rustiq`
input reconstructs and reruns its archived resolved calculation(s); multi-entry
bundles use the batch contract. Nickel imports resolve relative to the importing
file. Geometry paths resolve relative to the top-level input file; for TOML
read from standard input, they resolve from the current working directory.
Direct archive reruns cannot be combined with `--reuse`, `--artifact`,
`--cache-dir`, `--auto-download`, or `--no-auto-download`; the source bundle is
read-only and provides its resolved basis and inputs. Archive reruns do not use
the local ERI cache or basis-download settings. Text output shows SCF progress;
JSON output suppresses progress and writes only the result document.

Important options:

| Option | Behavior |
| --- | --- |
| `--format FORMAT` (`text` or `json`) | Select text output (default) or versioned JSON. JSON result data is written to standard output. |
| `--pretty` | Pretty-print single-calculation or batch JSON, optionally with syntax highlighting. Requires `--format json`. |
| `--color MODE` (`auto`, `always`, or `never`) | Control ANSI colors globally; also accepts `RUSTIQ_COLOR`. Use `--color never` when redirecting pretty JSON. |
| `--auto-download` / `--no-auto-download` | In online builds, allow or disable fetching a missing basis for this run. These override `RUSTIQ_AUTO_DOWNLOAD`; without either flag, `1` or `true` (case-insensitive) enables it and other or missing values disable it. Offline builds always use local bases. |
| `--cache-dir DIR` | Choose the AO ERI cache root for this run. The input must also set `[cache].enabled = true`; this option does not enable caching. |
| `--reuse PATH` | Read a portable `.rustiq` bundle and reuse the first compatible AO ERI for each calculation. |
| `--artifact PATH` | Write the current calculation or batch as a portable `.rustiq` bundle, independently of text or JSON output. |

Output presentation is selected exclusively by the CLI `--format text|json` option.
The TOML and Nickel calculation contracts do not accept an `output` section.
Both result presentations consume calculation events; JSON retains its single
versioned result document (including the existing batch envelope), without streaming
intermediate events. For Nickel and `.rustiq` batches, the CLI orchestrator
supplies all ordered outcomes through one campaign-completion event. The JSON
formatter builds the batch envelope from this object after execution; fatal
infrastructure errors prevent campaign completion and produce no batch document.

The cache directory path is resolved from the caller's current working
directory. Pass the same `--cache-dir` to `cache list` or `cache remove` to
manage entries from a run that used a custom cache root.

The CLI reports input, basis, preparation, and calculation errors as diagnostics.
Exit codes follow the Unix convention: zero means complete success; any nonzero
code means the command did not fully succeed. Codes 3 and 4 are RustiQ-specific.

| Code | Meaning |
| --- | --- |
| `0` | All requested calculations succeeded. Help and version requests also return zero. |
| `1` | General error, a batch with no successful calculations and at least one error unrelated to HF non-convergence, or a fatal infrastructure error. |
| `2` | Invalid command-line arguments. |
| `3` | No requested calculation succeeded, and every unsuccessful calculation stopped because HF did not converge. |
| `4` | A completed batch contains both successful and unsuccessful calculations, including non-convergence or recoverable errors. |

These codes apply to TOML, Nickel, and `.rustiq` inputs, with text or JSON output.
A calculation succeeds only when the complete requested method succeeds: converged
HF followed by an MP2 error is not a successful calculation. MP2 blocked by HF
non-convergence counts as non-convergence, so a single such run returns `3`.
Input validation before execution returns `1`; CLI argument validation returns `2`.
Fatal infrastructure errors, including output or artifact publication failures,
return `1` even if some calculations succeeded.

A single HF-only calculation that reaches its iteration limit still returns its
final-iteration result, with `converged: false` in JSON, but exits with `3`.
MP2 requires converged HF; a single MP2 run blocked by non-convergence reports a
diagnostic without result JSON. Recoverable per-entry errors in a batch do not
prevent later entries from running. The batch JSON records each outcome, and a
valid JSON document can accompany a nonzero exit code. Output fields remain
defined by the [versioned schemas](index.md#authoritative-references).
Convergence alone does not establish the physical validity of a result.

From the repository root, capture the status even when the calculation fails
(the conditional also works in shells using `set -e`):

```sh
status=0
cargo run -- run samples/h2/sto-3g/calculation.toml --format json > result.json || status=$?
printf 'Exit code: %s\n' "$status"
```

When using the installed binary, replace `cargo run --` with `rustiq`. In scripts,
inspect the exit code and, when available, individual JSON outcomes; see the
[JSON guide](../user-guide/json-output.md).

## Feature availability

| Capability | Available behavior |
| --- | --- |
| TOML | One calculation per input; see the checked-in [H₂ input](../user-guide/configuration.md). |
| Nickel | One calculation or an ordered study/batch; see the checked-in [study example](../user-guide/nickel-studies.md). |
| JSON | Versioned single-calculation and batch results, selected with `run --format json`; schemas are linked below. |
| AO ERI cache | Optional local reuse through `[cache].enabled` and `--cache-dir`; entries can be listed or removed. AO ERI means atomic-orbital electron-repulsion integral. |
| Portable `.rustiq` bundle | Create with `run --artifact`, rerun with `run FILE.rustiq`, reuse artifacts with `run INPUT --reuse FILE.rustiq`, inspect with `artifact inspect`, and validate stored AO ERIs with `artifact verify`. Reruns start a fresh HF calculation. See the [portable bundle guide](../user-guide/portable-bundles.md). |
| Scientific status | Experimental; selected checks do not establish research-grade correctness. See [scientific scope and limitations](../scientific-scope.md). |

Basis downloads and online basis listing require a build with the `online`
feature. The standard CLI enables it; offline builds provide import, local
listing, and removal. Geometry transformations are CLI utilities, not
calculation input transformations unless their output file is then used as an
input.

These examples use files shipped in the repository and must be run from its
root after checkout. The release archive contains neither fixtures nor sample
inputs. With only the binary, use `rustiq basis import /path/to/basis.json`
for a separately obtained basis file, or `rustiq basis download sto-3g` in an
online build; provide your own input file to `rustiq run`.

From the repository root, import the checked-in STO-3G and 6-31G basis
fixtures before running these examples:

```sh
rustiq basis import tests/data/sto-3g.json tests/data/reference/RustiQ/basis_sets/6-31g.json
rustiq run samples/h2/sto-3g/calculation.toml
rustiq run samples/h2/study.ncl --format json --pretty --color never
rustiq run samples/h2/sto-3g/calculation-cache.toml --cache-dir ./rustiq-cache
rustiq cache list --cache-dir ./rustiq-cache
```

The Nickel study uses **both STO-3G and 6-31G**; importing only STO-3G is
insufficient unless basis auto-download is enabled in an online build.
The TOML and cache examples require only STO-3G. See the
[Nickel studies guide](../user-guide/nickel-studies.md) for the individual
import commands. The relative `./rustiq-cache` directory is resolved from
the caller's working directory on Windows, Linux, and macOS.

The H₂ TOML and Nickel examples are exercised by
[`tests/cli_samples.rs`](https://github.com/mveril/RustiQ/blob/main/tests/cli_samples.rs),
[`tests/nickel_cli.rs`](https://github.com/mveril/RustiQ/blob/main/tests/nickel_cli.rs),
and [`tests/json_output_cli.rs`](https://github.com/mveril/RustiQ/blob/main/tests/json_output_cli.rs).
The JSON tests also check pretty-printing of Nickel batches. The
cache-enabled input is `samples/h2/sto-3g/calculation-cache.toml`; its
store-then-hit behavior is covered by
`test_cli_h2_sample_uses_eri_cache_when_enabled_in_runfile` in
[`tests/cli_samples.rs`](https://github.com/mveril/RustiQ/blob/main/tests/cli_samples.rs),
in addition to the Rust core persistence tests.

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
The format is conceptually inspired by [EZFIO](https://gitlab.com/scemama/EZFIO)
in [Quantum Package 2 (`qp2`)](https://github.com/QuantumPackage/qp2), but uses
an independent archive structure and APIs; see the
[comparison](https://github.com/mveril/RustiQ/blob/main/docs/persistence-format-v1.md#relationship-to-ezfio).
For a guided create, inspect, and reuse workflow, see the
[portable bundle guide](../user-guide/portable-bundles.md).
Bundles are separate from the disposable local AO ERI cache. Reuse opens its source
read-only. For each requested calculation, RustiQ selects the first compatible,
supported AO ERI in bundle order; when none is available, an enabled local cache
is checked before recomputation. A corrupt selected payload is an error. A
source entry can supply data to multiple calculations in a Nickel batch.

```sh
rustiq run samples/h2/sto-3g/calculation.toml --artifact h2.rustiq
rustiq run samples/h2/sto-3g/calculation.toml --reuse h2.rustiq --artifact next.rustiq --format json
rustiq artifact inspect h2.rustiq --format json
rustiq artifact verify h2.rustiq
rustiq run h2.rustiq
# Replace the source bundle after all requested calculations produce results:
rustiq run samples/h2/sto-3g/calculation.toml --reuse h2.rustiq --artifact h2.rustiq
```

Publication captures the current requested calculation collection and its
compatible known artifacts. It does not append history. An existing destination
is accepted only when it resolves to the explicit `--reuse` source; otherwise
the command fails without replacing it. Publication happens after every requested
calculation produces a result. Preparation or execution errors prevent
publication of the collection and leave the destination unchanged. An HF-only
non-converged result can contribute its AO ERI. A Nickel batch containing only
non-converged HF-only results can publish its bundle even though the command
exits unsuccessfully. If MP2 was requested but HF does not converge, execution
fails and the bundle is not published. HF orbitals, SCF restart state, DIIS
history, and MP2 results are not persisted.

When `--reuse` or `--artifact` is used, JSON calculation results include an
optional `artifacts` array. `decision` reports archive selection (`reused`,
`missing`, or `incompatible`); `origin` reports the ERI actually used
(`archive`, `cache`, or `computation`). A reused item includes its zero-based
`source_index`. Without these options the existing JSON result shape is retained.
`run FILE.rustiq --format json` uses the regular single-calculation or batch
result schema but does not include this reuse-decision array: the bundle is the
implicit artifact source for that mode.
Publication failures are reported on stderr and return a failing exit status;
JSON results for a completed calculation or batch may already have been written.

`artifact inspect PATH` displays requested and resolved scientific snapshots,
producer details, artifact representations, and captured source names and sizes.
In text mode, canonical TOML and XYZ are syntax-highlighted when color is
enabled by `--color` or `RUSTIQ_COLOR`; JSON output remains structured JSON.
It validates the archive index, provenance, snapshots, and declared artifact
sizes, but does not hash or decode numeric payloads. Use `artifact verify` to
check supported payloads eagerly; otherwise, execution checks a payload when
its scientific stage selects and reads it. Inspection does not execute
calculations or establish scientific validity. JSON output follows the
[artifact inspection V1 schema](https://github.com/mveril/RustiQ/blob/main/schemas/artifact-inspection-v1.schema.json).
`artifact verify PATH` eagerly reads and validates supported numerical
payloads. It currently checks compact AO ERI payload hashes, NPY encoding,
array dimensions, and consistency with the declared AO basis. It reports an
error for an invalid payload or unsupported representation. Verification does
not run a calculation or establish scientific validity.
The archive captures exact bytes of the main input and geometry files as shared
provenance; it does not capture Nickel's full import graph. See the
[persistence specification](https://github.com/mveril/RustiQ/blob/main/docs/persistence-format-v1.md)
for the archive contract and API details.

## Output contracts

`run --format json` selects one of two versioned contracts: the single
calculation schema or the batch schema. `schema_version` identifies the
contract; schemas, rather than this manual, define the complete field sets.
The [JSON guide](../user-guide/json-output.md) describes output selection,
standard output behavior, failure cases, and the existing automated checks.
