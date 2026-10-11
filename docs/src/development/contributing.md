# Contributing and the Rust API

[CONTRIBUTING.md](https://github.com/mveril/RustiQ/blob/main/CONTRIBUTING.md)
owns branch naming, lint policy, and verification commands.
See [development environments](environments.md) for setup.

The workspace keeps CLI parsing and presentation in the root package and
scientific configuration and calculations in `rustiq-chem-core`. The core's
`calculation` module orchestrates work in `molecules`, `basis`, `eri`,
`hf`, and `mp2`.

Rustdoc remains the canonical public API reference. Generate and open it with
MathJax support:

```sh
tools/generate-rustdoc.sh --open
```

On Windows:

```powershell
.\tools\generate-rustdoc.ps1 -Open
```

The generated entry point is `target/doc/rustiq_core/index.html`.
MathJax equations require browser access to the CDN.
The initial Pages site publishes the manual; rustdoc is generated separately.

[Runnable API examples](https://github.com/mveril/RustiQ/tree/main/crates/rustiq-core/examples)
and [public API tests](https://github.com/mveril/RustiQ/blob/main/crates/rustiq-core/tests/public_api.rs)
provide executable workflows and verification. Link to these sources instead
of duplicating item-level documentation in the book.

## Architecture and calculation lifecycle

The root CLI package owns terminal reports, application directories, and
`src/runfile/`, which resolves TOML/Nickel through the embedded Nickel schema.
The library owns scientific configuration and calculations without a Nickel
dependency. This boundary lets a different frontend supply scientific options
without embedding its input syntax or terminal behavior into numerical code.

Within the core, `molecules` owns geometry, units, charge and electron counts;
`basis` constructs Gaussian functions; `eri` computes and stores electron
repulsion integrals; `hf` solves RHF/UHF; and `mp2` consumes HF orbitals.
Rust makes ownership and mutation explicit for matrices, integral storage, and
shared results. Enums model choices such as the HF method, while Cargo brings
dependency locking, feature selection, and tests into one workflow. These
engineering tools help make assumptions inspectable; they do not establish
numerical correctness or a performance advantage over other languages.

Typed errors distinguish invalid input from numerical failures. Optional source
locations let the CLI attach diagnostics to input text without making the
library own that text or its presentation.

`CalculationBuilder` accepts geometry, basis data, and scientific options.
`prepare()` validates the state, converts coordinates to Bohr, constructs the
basis, and resolves RHF/UHF. A prepared calculation can execute repeatedly.
`run_hf()` distinguishes converged and unconverged outcomes; only the converged
solution exposes `mp2()`. This type boundary prevents accidental MP2 execution
on unconverged orbitals. Cloned solutions share immutable scientific data;
execution errors retain completed HF outcomes when available.

The normalized `CalculationRequest` records scientific input without geometry
paths or frontend source spans. Canonical TOML/XYZ rendering belongs to CLI
adapters. Core consumers supply explicit paths to `BasisStore::new`; application
storage environment variables are CLI policy. The core has no default features;
its optional `online` feature enables online basis support. See the runnable
examples and public API tests above for complete construction and error handling.

### Data preparation, execution, and reuse

The lifecycle is:

```text
CLI TOML/Nickel adapter or another frontend
    -> geometry + basis data + scientific options
    -> CalculationBuilder::prepare()
       molecule in Bohr, Gaussian basis, resolved HF method and random seeds
    -> PreparedCalculation::run_hf()
       one-electron integrals, overlap orthogonalization, AO ERI acquisition
       -> iterative SCF and final canonicalization
       -> HfOutcome::Converged or HfOutcome::Unconverged
    -> converged HfSolution::mp2()
       blocked AO-to-MO transformation and correlation energy
```

`prepare()` constructs reusable input data; it does not compute integrals or
solve HF. Each normal `run_hf()` call builds a new HF execution state, including
one-electron integrals and the orthogonalizer. Without an explicit reuse source,
AO ERIs are recomputed too. Repeated execution of a prepared calculation does
not itself memoize these numerical arrays. Random seeds resolved at preparation
are retained for repeated executions.

An optional `EriCache` can load compatible AO ERIs from a directory or store
newly computed ones. It is disposable local storage. A supplied `CompactEri`
through `run_hf_with_eri()` bypasses AO ERI computation and cache lookup after
compatibility checks. HF solutions retain their AO ERIs, and cloned solutions
share immutable result data; these retained results are distinct from the
prepared input data.

Portable `.rustiq` bundles use the core persistence APIs to save scientific
inputs and optional AO ERIs. Preparing a calculation from a bundle uses its
artifact compatibility policy to decide reuse; it does not turn a local cache
directory into a portable artifact. See the
[persistence contract](../reference/index.md) and runnable API examples for
restoration and explicit ERI transfer.

This distinction matters for performance: compact AO ERI storage still grows
as the fourth power of basis dimension, and the MP2 workspace budget covers
additional transformation buffers rather than the full HF data or process
memory. See [MP2 memory policy](../reference/existing-workflows.md#mp2-and-its-memory-budget).

## Build profiles and performance investigation

Cargo's default development and test builds favor development speed. Release
builds optimize execution using thin link-time optimization and one codegen unit,
and strip symbols. The custom `profiling` profile retains release optimizations
with debug information and symbols:

```sh
cargo build --locked --release
cargo build --locked --profile profiling
```

The profiling executable is under `target/profiling/`. For optimization specific
to the build machine's CPU, use `RUSTFLAGS="-C target-cpu=native" cargo build
--locked --release`; that binary may require instructions absent on another CPU.
In PowerShell, set `$env:RUSTFLAGS = "-C target-cpu=native"`, build, then remove
it with `Remove-Item Env:RUSTFLAGS`. There is no separate `native` Cargo profile.

The `eri_timings` and `mp2_timings` benchmarks use Criterion and require
`bench-support`:

```sh
cargo bench --bench mp2_timings --features bench-support
cargo bench --bench eri_timings --features bench-support
```

The MP2 benchmark compares dense and memory-blocked kernels on deterministic
synthetic inputs. It validates that both paths produce matching energies before
timing them; input and ERI construction remain outside the measured iterations.
`RUSTIQ_MP2_MEMORY`, `RUSTIQ_MP2_SIZES` (comma-separated AO dimensions), and
`RAYON_NUM_THREADS` select the memory budget, AO dimensions, and thread count.
The ERI benchmark times repeated integral construction for the checked-in
molecular cases. By default it skips the larger ethanol and cholesterol cases;
set `RUSTIQ_BENCH_HEAVY=1` to include them. `RUSTIQ_BENCH_FILTER` selects cases
by name or geometry path, and `RUSTIQ_BENCH_BASIS_STORE` selects a basis store.
It reports compact ERI storage elements processed per second. This count is the
size of the compact representation; Schwarz screening may skip evaluations or
leave stored elements zero, so it is not a count of integrals evaluated per
second.

The standard cases use 30 samples, a 2-second warm-up, and a 5-second
measurement target. Heavy ERI cases and MP2 cases with 48 or more AOs use
Criterion's minimum of 10 samples, a 1-second warm-up, and a 2-second target.
These are targets: Criterion may take longer to collect the requested samples,
especially when each calculation is expensive. Heavy ERI cases remain disabled
unless `RUSTIQ_BENCH_HEAVY=1` is set. Measurements are saved under
`target/criterion`.

Criterion baselines can be compared only between revisions that both use
Criterion. The first Criterion-enabled revision establishes the starting point;
legacy timing results cannot be compared directly with Criterion measurements.
MP2 input construction is excluded from measured iterations. Keep CPU
architecture, Rayon thread count, build flags, enabled features, MP2 memory
budget, power settings, and benchmark environment variables consistent. Save a
baseline on the first Criterion revision, then compare from the revision under
test:

```sh
cargo bench --bench mp2_timings --features bench-support -- --save-baseline before
cargo bench --bench mp2_timings --features bench-support -- --baseline before
```

See the [MP2 benchmark source](https://github.com/mveril/RustiQ/blob/main/benches/mp2_timings.rs)
and [ERI benchmark source](https://github.com/mveril/RustiQ/blob/main/benches/eri_timings.rs).
Benchmarks provide performance measurements, not scientific validation.
