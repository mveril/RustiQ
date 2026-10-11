# Portable calculation bundles

A `.rustiq` bundle is a portable snapshot of RustiQ calculation inputs and
reusable atomic-orbital electron-repulsion integrals (AO ERIs). It lets later
calculations reuse compatible AO ERIs without carrying a machine-local cache.
The bundle does not save a Hartree–Fock wavefunction or skip a new SCF run.

## Create and inspect a bundle

Run the examples from the repository root. First make the checked-in STO-3G
basis available, then run the H₂ sample and write a bundle to the current
directory:

```sh
cargo run --locked -- basis import tests/data/sto-3g.json
cargo run --locked -- run samples/h2/sto-3g/calculation.toml --artifact h2.rustiq
```

Choose a destination that does not already exist. `--artifact` writes the
current calculation collection as a new bundle, independently of text or JSON
result output. For a Nickel study, the bundle contains one entry for each
requested calculation.

Inspect a bundle without the original runfile, geometry file, or basis store:

```sh
cargo run --locked -- artifact inspect h2.rustiq
cargo run --locked -- artifact inspect h2.rustiq --format json
```

Inspection displays canonical requested and resolved calculation snapshots,
producer information, artifact representations, and captured source names and
sizes. Requested values may differ from resolved values: for example, an
`Auto` HF method can resolve to RHF, and requested Angstrom coordinates appear
as resolved Bohr coordinates. The optional JSON `payloads_verified` field is
`false`: inspection validates the archive index, provenance, scientific
snapshots and declared artifact sizes, but does not hash or decode numerical
payloads. A payload is fully checked when execution selects and reads it.
Inspection is structural, not a calculation or an independent scientific
validation.

## Reuse AO ERIs in a new calculation

The CLI still needs a TOML or Nickel input, its geometry, and an available basis.
`--reuse` does not run the calculation saved in the bundle or restore its
captured source files. Instead, RustiQ prepares the supplied input, checks the
bundle for compatible AO ERIs, and runs HF again; it also runs MP2 again if the
new input requests MP2.

```sh
cargo run --locked -- run samples/h2/sto-3g/calculation.toml \
  --reuse h2.rustiq --format json --pretty --color never
```

The JSON result includes an `artifacts` entry when either portable artifact
option is used. A reused AO ERI is reported like this:

```json
{
  "artifacts": [
    {
      "name": "ao_eri",
      "decision": "reused",
      "origin": "archive",
      "source_index": 0
    }
  ]
}
```

`decision` says whether the bundle supplied a compatible artifact; the CLI
reports `reused`, `missing`, or `incompatible`. `origin` says where the ERIs
actually came from: `archive`, an enabled local `cache`, or fresh `computation`.
For example, `decision: incompatible` with `origin: cache` means no bundle entry
matched but an enabled local cache supplied the ERIs. When an archive entry is
selected, `source_index` is its zero-based position in that bundle. In a batch,
each result reports its own selection.

The reuse order is:

1. First compatible AO ERI in the archive's manifest order.
2. Matching entry in the enabled local AO ERI cache.
3. Recompute the AO ERIs.

The local-cache step is used only when `[cache].enabled = true` in the runfile.
`--cache-dir` selects the cache location but does not enable caching; see the
[configuration guide](configuration.md#optional-local-ao-eri-cache) for the
input setting.

The archive remains read-only during reuse. To publish a new snapshot from the
calculation, provide a new destination:

```sh
cargo run --locked -- run samples/h2/sto-3g/calculation.toml \
  --reuse h2.rustiq --artifact h2-updated.rustiq
```

## What can be reused?

AO ERIs are reusable when the effective integral inputs match. RustiQ checks the
ordered nuclear coordinates in Bohr, effective AO basis functions, Schwarz
screening setting, and integral representation and computation versions. A
matching basis-set label or atom count alone is not enough. HF method and
convergence settings, and whether MP2 is requested, do not change AO ERI
compatibility. The [persistence specification](https://github.com/mveril/RustiQ/blob/main/docs/persistence-format-v1.md#scientific-identity-v1)
defines the complete identity.

| Change to the new input | AO ERI compatibility |
| --- | --- |
| HF method, initial guess, or SCF convergence limit | Can remain compatible; HF runs again with the new settings. |
| Add or change MP2 settings | Can remain compatible; MP2 runs again from the new HF result. |
| Geometry, effective AO basis, or Schwarz screening | Incompatible; RustiQ checks the local cache, then recomputes if needed. |

The bundle stores scientific input snapshots and optional AO ERIs. It does not
store HF energies, convergence status, orbitals, density matrices, DIIS history,
or MP2 results. Save the result JSON separately when those outputs matter.
Captured input and geometry bytes are provenance for inspection; they do not
affect compatibility, and Nickel imports are not captured recursively.
Rust callers can reconstruct a calculation from the archived scientific
snapshots without a runfile; see the Rust API examples in the
[persistence specification](https://github.com/mveril/RustiQ/blob/main/docs/persistence-format-v1.md#single-calculation-execution-apis).

## Publication and failures

`--artifact` publishes the current requested collection; it does not append to
the source bundle. A new destination must not exist. An existing destination is
accepted only when it resolves to the explicit `--reuse` source, in which case
RustiQ replaces that bundle after producing a complete validated snapshot.
Other existing destinations are left untouched.

| Run outcome | Bundle behavior |
| --- | --- |
| Every requested calculation produces a result | Publish the current collection. |
| A preparation or execution error occurs | Do not publish; preserve any prior destination. |
| HF-only run reaches its iteration limit | Its result can still publish the AO ERIs. A single HF run exits successfully; a batch with non-converged results and no execution errors publishes, then exits unsuccessfully. |
| MP2 was requested but HF did not converge | MP2 execution fails, so the collection is not published. |

JSON for a completed calculation or batch may already have been written when a
later publication error is reported. See the [CLI reference](../reference/cli.md#cache-and-portable-bundles)
for exact exit and publication behavior and the
[persistence V1 specification](https://github.com/mveril/RustiQ/blob/main/docs/persistence-format-v1.md)
for the archive contract and the [EZFIO comparison](https://github.com/mveril/RustiQ/blob/main/docs/persistence-format-v1.md#relationship-to-ezfio).
