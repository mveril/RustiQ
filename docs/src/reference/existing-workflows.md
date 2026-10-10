# Existing calculation workflows

This page preserves operational guidance from the former README while the
practical guides are developed. Commands below run from the repository root;
Cargo builds and launches the CLI. An installed binary can replace `cargo run
--locked --` with `rustiq` (the executable name is case-sensitive).
See the [glossary](../glossary.md) for scientific and development terms.

## Create input from an XYZ geometry

```sh
cargo run --locked -- init samples/h2/molecule.xyz -o calculation.toml
cargo run --locked -- init samples/h2/molecule.xyz --hf uhf --multiplicity 3 --mp2 -o triplet.toml
```

`init` references the geometry; it does not copy it. The output defaults to
`calculation.toml` in the current directory. Its parent directory must exist.
Use `--force` to replace an existing regular file; the source geometry,
directories, and symbolic links are protected from replacement.

Defaults are STO-3G, neutral charge, Angstrom coordinates, and automatic HF
selection. Without `--multiplicity`, the electron count (including charge)
selects a singlet for an even count and a doublet for an odd count. This is a
starting convention, not a prediction of the ground state. Specify the
physical state yourself. `--hf` accepts `auto`, `rhf`, or `uhf`; `--units`
accepts `angstrom` or `bohr`. Incompatible molecular states are rejected.
`--basis` and `--charge` select another basis and charge. `--mp2` adds MP2 with
no frozen orbitals. Default-valued settings are omitted from generated TOML
and restored by the authoritative [Nickel configuration](index.md).

Initialization needs neither a cached basis nor network access. Running the
calculation checks basis availability and numerical requirements.
[tests/init_cli.rs](https://github.com/mveril/RustiQ/blob/main/tests/init_cli.rs)
checks that generated HF and MP2 inputs run; unit tests in
[init_command.rs](https://github.com/mveril/RustiQ/blob/main/src/cli/commands/init_command.rs)
check options, paths, defaults, and overwrite protection.

Geometry paths resolve from the top-level calculation file's directory,
including geometry values supplied by Nickel imports. Nickel imports themselves
resolve from the importing file. With no input filename, `run` reads TOML from
standard input and resolves geometry from the caller's current directory.
CLI paths such as `--cache-dir` also resolve from the caller's directory.

## Basis management and storage

A basis set supplies the functions used to represent electronic orbitals.
Import the checked-in fixture for the [first calculation](../getting-started/first-calculation.md),
or manage the local store with:

```sh
cargo run --locked -- basis list
cargo run --locked -- basis import tests/data/sto-3g.json
cargo run --locked -- basis list --online
cargo run --locked -- basis download sto-3g
cargo run --locked -- basis remove sto-3g
```

Online listing and download require the `online` Cargo feature, enabled by
default for the CLI. Offline builds retain import, local listing, and removal.
`run --auto-download` permits downloading a missing basis;
`--no-auto-download` disables it. These flags override `RUSTIQ_AUTO_DOWNLOAD`,
which enables downloading for `1` or `true` (case-insensitive); otherwise it is
disabled. Import and store behavior are tested in
[basis_store.rs](https://github.com/mveril/RustiQ/blob/main/crates/rustiq-core/src/basis/basis_store.rs).

The CLI uses the platform's local application data directory, then appends
`basis_sets`:

| Platform | Default basis directory                           |
| -------- | ------------------------------------------------- |
| Linux    | `~/.local/share/rustiq/basis_sets`                |
| Windows  | `%LOCALAPPDATA%\rustiq\data\basis_sets`           |
| macOS    | `~/Library/Application Support/rustiq/basis_sets` |

An absolute `XDG_DATA_HOME` replaces `~/.local/share` on Linux.
`RUSTIQ_DATA_BASIS` overrides the entire basis directory and takes precedence
over `RUSTIQ_DATA_HOME`, which selects `<root>/rustiq/basis_sets`.
Prefer absolute paths for these overrides. If the platform directory cannot be
determined, the fallback is `<system temporary directory>/rustiq/basis_sets`;
the operating system may remove its contents.

Older alpha installations used `~/.local/share/RustiQ/basis_sets` on Linux,
`%LOCALAPPDATA%\RustiQ\basis_sets` on Windows, and
`~/Library/Application Support/RustiQ/basis_sets` on macOS. There is no
automatic migration or lookup there. Copy existing bases to the current
location or set `RUSTIQ_DATA_BASIS` to the old directory. The policy and
override tests live in
[directories.rs](https://github.com/mveril/RustiQ/blob/main/src/cli/directories.rs).
The basis store is separate from the disposable integral cache and portable
scientific bundles described under [authoritative references](index.md).

## JSON results and Nickel studies

```sh
cargo run --locked -- run samples/h2/sto-3g/calculation.toml --format json
cargo run --locked -- run samples/h2/sto-3g/calculation.ncl
cargo run --locked -- run samples/h2/study.ncl --format json
```

The study runs STO-3G and 6-31G calculations sequentially. Import both fixtures
from `tests/data/reference/RustiQ/basis_sets/`, or permit automatic download.
The input extension selects TOML or Nickel; no external Nickel executable is
needed. TOML and `init` describe single calculations.

JSON is suitable for automated analysis because energies retain floating-point
precision rather than terminal display rounding. Use the existing
[single and batch schemas](index.md) as the field contracts; check
`schema_version`. UHF results include spin expectation and contamination
relative to the requested spin state; RHF results omit that spin object.
Unconverged results describe the final iteration, not a converged wavefunction.

One calculation, including an array of length one, produces the single-result
contract. Larger arrays produce the batch contract in source order, with
`success`, `non_converged`, or `error` outcomes. Configuration is validated
before execution. Runtime calculation failures allow later entries to run;
any failure or nonconvergence makes the batch exit unsuccessfully while its
JSON remains a complete document. Empty arrays are rejected.

Compact JSON has no ANSI styling even with forced color. `--pretty` requires
`--format json` and can add syntax highlighting when colors are enabled;
use `--color never` when saving pretty JSON for another program.
[JSON tests](https://github.com/mveril/RustiQ/blob/main/tests/json_output_cli.rs)
and [Nickel tests](https://github.com/mveril/RustiQ/blob/main/tests/nickel_cli.rs)
verify these contracts and study execution.

## MP2 and its memory budget

MP2 estimates a correlation correction using converged HF orbitals; RustiQ
supports both RHF and UHF references. It refuses MP2 on unconverged HF.
The existing H₂ input is:

```toml
{{#include ../../../samples/h2/sto-3g/mp2_calculation.toml}}
```

After importing STO-3G, run:

```sh
cargo run --locked -- run samples/h2/sto-3g/mp2_calculation.toml
```

`frozen_orbitals` excludes the lowest occupied orbitals from correlation;
zero correlates all occupied orbitals. For RHF, it counts spatial orbitals:
`frozen_orbitals = 1` freezes one doubly occupied orbital (two electrons).
For UHF, the same count applies separately to the alpha and beta occupied
spaces: `1` freezes the lowest occupied alpha orbital and the lowest occupied
beta orbital, also excluding two electrons. These spin orbitals can have
different spatial shapes. The CLI option does not specify separate spin counts
or arbitrary orbital indices.

RHF requires the count to be smaller than the occupied-orbital count. UHF
requires it not to exceed either spin's occupied count; a spin channel may have
no active occupied orbitals left. Choose the frozen space explicitly when
comparing packages. Defaults and input validation belong to [Nickel](index.md);
the core also checks the reference-specific orbital partition during MP2.
The [PySCF comparisons](../scientific-scope.md#reference-comparisons)
include RHF/UHF MP2, frozen orbitals, and different workspace budgets.

`memory_limit = "auto"` resolves once before MP2 to half the available memory.
On Linux, the process's cgroup free-memory limit caps the host value when
available. If memory information is unavailable, the fallback is 512 MiB.
Explicit sizes such as `"500 MB"` (decimal units) and `"1.5 GiB"` (binary units)
are accepted. The budget covers additional matrix payloads for the blocked
transformation from atomic to molecular orbitals, including panels and
coefficient copies. HF data and compact atomic-orbital integrals are separate.
It does not cap total process memory, allocator overhead, or matrix-kernel
scratch storage. A budget too small for one occupied block fails before the
transformation buffers are allocated. The text report shows block size and
workspace estimates. See the tested
[resource policy](https://github.com/mveril/RustiQ/blob/main/crates/rustiq-core/src/resources.rs)
and [blocked implementation](https://github.com/mveril/RustiQ/blob/main/crates/rustiq-core/src/mp2/blocked.rs).

## Terminal colors

`--color` and `RUSTIQ_COLOR` accept `auto`, `always`, and `never`, in order of
precedence: CLI flag, environment setting, automatic detection. In automatic
mode, `NO_COLOR` disables styling; otherwise terminal detection is independent
for standard output (reports) and standard error (diagnostics and progress).
Explicit `always` or `never` overrides `NO_COLOR`.

```sh
cargo run --locked -- --color never run samples/h2/sto-3g/calculation.toml
```

Windows output uses adaptive terminal support, including legacy consoles.
The implementation and focused checks are in
[color.rs](https://github.com/mveril/RustiQ/blob/main/src/cli/color.rs)
and the JSON integration tests. See the JSON section above for pretty output.
