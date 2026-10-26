# Contributing and the Rust API

[CONTRIBUTING.md](https://github.com/mveril/RustiQ/blob/main/CONTRIBUTING.md)
owns branch naming, lint policy, and verification commands.
See [development environments](environments.md) for setup.

Run `cargo deny check` from the workspace root to check dependency advisories,
licenses, duplicate versions, and dependency sources. The same check runs in a
dedicated GitHub Actions job; its policy is defined in `deny.toml`.

The distribution script requires Python with `tarfile` extraction filters
(Python 3.12+, or patched 3.10.12+/3.11.4+). The `rust` and `full` Nix shells
provide cargo-about. Outside Nix, install cargo-about 0.9.2. After dependency
or notice-generation changes, run `python3 tools/distribution.py notices`, then
`python3 tools/distribution.py check`. Commit the generated notice text and its
checksum with the change. See [licenses and redistribution](licensing.md) for
the distribution policy and Nickel's dependency advisories.

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
The Pages site publishes the manual; rustdoc is generated separately.

[Runnable API examples](https://github.com/mveril/RustiQ/tree/main/crates/rustiq-core/examples)
and [public API tests](https://github.com/mveril/RustiQ/blob/main/crates/rustiq-core/tests/public_api.rs)
provide executable workflows and verification. Link to these sources instead
of duplicating item-level documentation in the book.

## Public API imports

Use `rustiq_core::prelude::*` for direct calculations. It includes
`CalculationExecution`, the trait providing `execute()` and
`execute_with_events()` for builders and prepared calculations. With explicit
imports, bring `rustiq_core::calculation::CalculationExecution` into scope to
call those methods. The builder supports mutable setters such as `hf(...)`,
accessors such as `get_hf()`, and consuming `with_hf(...)` variants.

Explicit imports use `molecules` for molecular inputs, `config` for options,
random distributions and validated scalars, `basis` for basis data, and
`calculation` for execution and results. `Located<T>` remains available from
`config` and the prelude for optional frontend source positions.

Extensible option structs, including HF, DIIS, orthogonalization, molecular,
integral, MP2, random-guess, and guess-perturbation configurations, are
`#[non_exhaustive]`. Create them with `Default::default()` and then assign the
public fields you want to customize. External Rust callers cannot use struct
literals or `..Default::default()` to construct these types. See the core
`config` module's Rustdoc for executable examples.

`RandomConfig` is also non-exhaustive, but has no `Default` implementation
because no distribution is appropriate for every calculation. Start with
`RandomConfig::new(distribution)` and assign `seed` when reproducible sampling
is required. An absent seed continues to initialize sampling from system
randomness.

New options must have defaults that preserve the previous behavior when the
option is omitted. Maintain this contract in both the core's `Default`
implementations and the input frontend's defaults. Non-exhaustiveness protects
Rust source compatibility when fields are added; preserving default behavior
also requires checking the existing calculation and input regression tests.

The core produces `ScfResult`, `ScfIteration`, SCF timings,
`HfCalculationResult`, and `Mp2MemoryPlan`; these are non-exhaustive inspection
records, not extension points for constructing scientific output. Public
fields remain readable, while new diagnostics can be added without requiring
downstream struct literals to change. `Mp2Result` remains constructible because
the public `Mp2Calculation` trait lets downstream implementations return the
same result type.

Enums are non-exhaustive when callers select among evolving configurations or
consume results that may gain another variant, such as `HfMethod`,
`ResolvedHfMethod`, density guesses, random distributions, and MP2 sectors.
`HfOutcome` remains exhaustive because a completed HF run has exactly two
convergence states; setup and calculation failures are returned as errors.
`RestrictedMethod` and `UnrestrictedMethod` also remain exhaustive and name
only the currently implemented method for each reference. Adding a supported
method is an explicit API change, rather than an open-ended configuration
choice.

For portable archives and raw AO ERIs, import `RustiQData`, `RustiQBundle`, and `CompactEri` explicitly from
`rustiq_core::persistence`. Import `ArtifactReuseDecision` and
`ArtifactReuseEvent` from `rustiq_core::calculation`; these advanced types
require explicit imports. The portable examples linked above demonstrate the
persistence imports, and the
[portable prelude test](https://github.com/mveril/RustiQ/blob/main/crates/rustiq-core/tests/prelude_portable.rs)
shows how to combine them with the calculation prelude.

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

UHF builds the alpha and beta Fock matrices in one pass over AO pairs. Each
pair shares the Coulomb contribution from the total density and the integral
loads, while retaining a separate exchange contribution for each spin density.
The independent scalar contraction regression covers unequal spin densities,
equal spin densities, and an empty beta density across different Rayon pools:
`cargo test -p rustiq-chem-core spin_fock_matrices_match_independent_contractions`.

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

The core emits `CalculationCompleted` once after all requested stages complete.
An HF-only run emits it even when the returned HF outcome is unconverged; execution
completion and convergence are separate facts. A failing requested MP2 stage leaves
the earlier HF notification available but emits no calculation completion. Callbacks
borrow results synchronously; retaining them requires copying.
The CLI copies only numerical summaries and artifact reports for batch presentation,
so collecting a campaign does not retain orbital matrices or AO integrals.
Campaign execution returns one ordered `CampaignResult` directly, including
recoverable errors. Fatal errors stop execution before presentation. The CLI then
derives the campaign summary, publishes artifacts when allowed, and renders the
complete text or versioned JSON document. Publication failure after successful
scientific execution still leaves the completed JSON on stdout while returning a
failing process status. Portable bundle reruns use the same result presentation.

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
       -> iterative SCF; final canonicalization if converged
       -> HfOutcome::Converged or HfOutcome::Unconverged
    -> extract the typed converged RHF/UHF solution
    -> Mp2Calculation::mp2()
       blocked AO-to-MO integral transformation and MP2 correlation-energy evaluation
```

`HfOutcome` records whether HF converged and contains an
`AnyHfSolution<State>` whose `Restricted` or `Unrestricted` variant retains the
reference-specific solution. `HfSolution<R, State>` carries both the reference
and convergence state in its type; the `RhfSolution` and `UhfSolution` aliases
name the common profiles. `AnyHfSolution` does not expose MP2. For HF followed
by MP2, call `prepared.run_hf()?.into_converged()?`, match the runtime variant,
then call `mp2(config)` on the resulting typed RHF or UHF solution. Both typed
solutions implement the `Mp2Calculation` capability trait, exported in the
prelude; unconverged and generic runtime solutions do not. The conversion
returns an execution error retaining the unconverged outcome, recoverable with
`into_hf()`. Typed/runtime conversions preserve state and return a typed
`HfReferenceMismatch` error that retains the original solution when the requested
reference does not match.

The owned-output implementation lives in
`crates/rustiq-core/src/calculation/solution/`, with separate modules for generic
results, HF adaptation, runtime dispatch, outcomes and execution errors. The
`calculation/solution.rs` facade preserves the existing public exports through
`rustiq_core::calculation` and the prelude. Convergence markers stay together,
and private shared storage stays with the result that owns it.

`HfSolution<R, State>` wraps `ScfSolution<HfMethodDescriptor<R>, State>`.
The following conversions preserve the physical method, reference, convergence
state and shared scientific allocation:

| Source | Target | Conversion |
| --- | --- | --- |
| `HfSolution<R, State>` | `ScfSolution<HfMethodDescriptor<R>, State>` | `From` / `Into`, or `into_scf()` |
| `ScfSolution<HfMethodDescriptor<R>, State>` | `HfSolution<R, State>` | `From` / `Into`, or `from_scf()` |
| `&HfSolution<R, State>` | `&ScfSolution<HfMethodDescriptor<R>, State>` | `AsRef`, or `as_scf()` |
| Restricted or unrestricted generic HF output | `AnyHfSolution<State>` | `From` / `Into` |
| `AnyHfSolution<State>` | Restricted or unrestricted generic HF output | `TryFrom` / `TryInto` |

The checked runtime extraction returns `HfReferenceMismatch<State>` on a
reference mismatch. It reports the expected and actual HF references and retains
the original `AnyHfSolution<State>`, recoverable with `into_solution()`. These
conversions transfer or borrow existing ownership; they do not clone scientific
data or allocate another result. The named methods delegate to the standard
traits. The HF wrapper keeps its explicit interface, including
`occupied_orbitals()`; existing HF callers need no migration.

`ScfSolution<M, State>` owns scientific output; `ScfResult` contains numerical
metrics. The public `ScfMethod` trait associates independent coefficient,
energy, occupation, summary and `RetainedData` types. It describes output rather
than an executable SCF algorithm. The implementing descriptor type identifies
the physical method separately from those representations. The generic `occupations()` accessor
borrows method-specific occupations, which need not be integer orbital counts.
`retained_data()` borrows scientific artifacts retained after execution; the
generic contract requires neither an HF summary nor AO ERIs.

Implementing this trait does not provide an SCF engine or a public solution
constructor: the library establishes the method-specific convergence state.
For HF, `HfMethodDescriptor<R>` identifies the compatible method, with an
`HfCalculationResult` summary and retained AO ERIs. Its `ScfSolution`
specialization exposes `scf_result()` to borrow metrics from the summary.
Only compatible converged HF wrappers implement `Mp2Calculation`; generic HF
output can be restored through `Into` or `from_scf()`. `AnyHfSolution` itself
does not imply that MP2 is available for every contained or future HF reference.
Supported converged HF implementations also guarantee final canonicalization;
this is an HF guarantee, rather than a universal requirement on every SCF
method.

The sealed `HfReference` contract associates a compatible HF method, coefficient
representation, orbital energies, and occupations. Generic code converts the
associated method into `ResolvedHfMethod`; conversion into the input selector
`HfMethod` is not required by the trait. RHF has one shared
coefficient matrix and energy vector with one doubly occupied spatial-orbital
count. UHF has separate alpha and beta coefficients, energies, and electron
counts, represented by `Spin<T>`. Read-only accessors expose these values;
energies are in Hartree, and each occupation count selects a prefix of its
coefficient columns. Cloning a solution shares retained scientific data. An
unconverged solution keeps diagnostic orbitals, while final canonical orbitals
are guaranteed only after convergence.

Initial guesses use the same reference markers through `DensityGuess<R>` and
`OrbitalGuess<R>`. Reference-specific Fock-like inputs contain one matrix for
`Restricted` and a `Spin` pair for `Unrestricted`, using the same `HfReference`
component contract as `HfSolution`. The zero startup sentinel is available for
both. Unperturbed guesses retain one core matrix for RHF or duplicate it for
UHF. Random and perturbed guesses generate one matrix for RHF or two ordered
alpha/beta matrices for UHF without constructing a discarded beta matrix;
the seeded alpha sequence is preserved across references.

Only converged solutions expose MP2. Method dispatch is exhaustive, so each
supported reference has an explicit correlation implementation. An MP2 error
retains the completed HF result for inspection or retry. See the
[`hf_solution` example](https://github.com/mveril/RustiQ/blob/main/crates/rustiq-core/examples/hf_solution.rs)
and [API tests](https://github.com/mveril/RustiQ/blob/main/crates/rustiq-core/tests/hf_solution.rs)
for executable workflows.

The complete offline example in `crates/rustiq-core/examples/hf_solution.rs`
is also included directly in the `HfOutcome` API documentation. Run it from the
repository root:

```sh
cargo run -p rustiq-chem-core --example hf_solution --no-default-features
```

It embeds the checked-in STO-3G basis and the H2 singlet and OH doublet geometries,
with coordinates interpreted in Angstrom. It prints reference-specific occupied
counts, total HF energies and MP2 correlation energies in Hartree. A deliberately
invalid frozen-orbital count demonstrates error recovery: the example drops the
original HF handle, extracts the retained output from the error using the standard
conversion traits, and retries MP2 without rerunning HF. Unexpected failures
propagate as errors. The example illustrates API ownership and error handling;
the independent scientific comparisons remain separate verification procedures.

`prepare()` constructs reusable input data; it does not compute integrals or
solve HF. Each normal `run_hf()` call builds a new HF execution state, including
one-electron integrals and the orthogonalizer. Without an explicit reuse source,
AO ERIs are recomputed too. Repeated execution of a prepared calculation does
not itself memoize these numerical arrays. Random seeds resolved at preparation
are retained for repeated executions.

An optional `EriCache` can load compatible AO ERIs from a directory or store
newly computed ones. It is disposable local storage. A supplied `CompactEri`
through `run_hf_with_eri()` bypasses AO ERI computation and cache lookup, but
the raw tensor API checks its expected element count rather than proving its
scientific provenance. The caller must supply ERIs for the prepared geometry,
AO ordering, and screening policy. Use the persistence APIs' compatibility
checks when reusing an archive. HF solutions retain their AO ERIs, and cloned
solutions share immutable result data; these retained results are distinct from
the prepared input data.

Portable `.rustiq` bundles can be read and written through the core persistence
APIs and through the CLI's `run --reuse`, `run --artifact`, `artifact inspect`,
and `artifact verify` commands. `run INPUT --reuse BUNDLE` resolves the supplied
runfile's geometry and basis as usual, then uses bundle compatibility to select
AO ERIs. `run BUNDLE.rustiq` reconstructs the archived resolved calculations
without the original inputs or local basis store. The Rust API can also reconstruct
a calculation from those scientific snapshots. These workflows start fresh HF
executions; HF solutions and SCF restart state are not persisted. Neither workflow turns
a local cache directory into a portable artifact. See the
[portable bundle guide](../user-guide/portable-bundles.md), the
[persistence contract](../reference/index.md), and the runnable API examples
for details.
Portable NPY ERI payloads stream directly into the final shared compact storage
through an exact-size iterator, avoiding a temporary full-size `Vec<f64>`.

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

Coulomb integrals cache Boys function values by order. When the requested
maximum order is nonzero, positive arguments up to 20 use a positive-term series
to seed the highest order, followed by downward recurrence; larger arguments
retain the gamma-function seed. An order-zero cache uses the error-function
expression. This changes numerical evaluation rather than the integral
definition or screening policy. The unit tests in
[`boys.rs`](https://github.com/mveril/RustiQ/blob/main/crates/rustiq-core/src/math_utils/boys.rs)
compare the cache against independent quadrature, including the series boundary.
Run `cargo test -p rustiq-chem-core math_utils::boys` and the affected
[PySCF comparisons](https://github.com/mveril/RustiQ/blob/main/tools/reference/README.md) after changing this path.

The `eri_timings` and `mp2_timings` benchmarks use Criterion and require
`bench-support`:

```sh
cargo bench --bench mp2_timings --features bench-support
cargo bench --bench eri_timings --features bench-support
cargo bench --bench eri_storage_allocation --features bench-support
cargo bench --bench eri_storage_memory --features bench-support
```

The MP2 benchmark compares the dense AO-to-MO transformation and correlation
energy evaluation with the production memory-blocked path on deterministic
synthetic RHF inputs. Each blocked case is constrained to process occupied
orbitals in at least two blocks, even when the requested memory budget could fit
one block. `RUSTIQ_MP2_MEMORY` sets an upper cap; each case may use a smaller
budget to force multiple blocks. AO dimensions must be at least 6. It checks that
both paths produce matching energies before timing
them, and records the selected block size, block count, workspace budget, and
estimated temporary workspace. Input and ERI construction remain outside the
measured iterations. These synthetic inputs benchmark the contraction code;
they are not molecular calculations or scientific accuracy validation.
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

The `eri_storage_allocation` benchmark compares Vec-then-Arc construction with
the exact-size-iterator Arc initialization path for identical compact tensor
sizes. It measures storage allocation and initialization only, not ERI
evaluation. Compare the same Criterion benchmark and compiler settings on both
revisions. The Miri
workflow targets a sequential unit test for the shared in-place initialization
helper, checking its initialization safety independently of Rayon scheduling.
Run the same check locally with nightly Rust:

```sh
rustup +nightly component add miri rust-src
cargo +nightly miri setup
cargo +nightly miri test -p rustiq-chem-core --lib test_in_place_storage_initialization_miri
```

The `eri_storage_memory` benchmark uses a counting global allocator in a
separate executable to report peak live heap bytes attributable to each
construction. It includes requested allocation sizes, not allocator overhead
or process RSS. It compares the same Vec-then-Arc and in-place storage paths.

The standard cases use 30 samples, a 2-second warm-up, and a 5-second
measurement target. Heavy ERI cases and MP2 cases with 48 or more AOs use
Criterion's minimum of 10 samples, a 1-second warm-up, and a 2-second target.
These are targets: Criterion may take longer to collect the requested samples,
especially when each calculation is expensive. Heavy ERI cases remain disabled
unless `RUSTIQ_BENCH_HEAVY=1` is set. Measurements are saved under
`target/criterion`.

Criterion baselines can be compared only between revisions that both use
Criterion and the same benchmark workload. The multiblock MP2 workload has a
new Criterion identifier, so its first run establishes a new starting point;
do not compare it with the previous single-block `mp2/blocked` measurements.
Legacy timing results also cannot be compared directly with Criterion measurements.
MP2 input construction is excluded from measured iterations. Keep CPU
architecture, Rayon thread count, build flags, enabled features, MP2 memory
budget, power settings, and benchmark environment variables consistent. Save a
baseline on the reference revision with the same workload, then compare from
the revision under test:

```sh
cargo bench --bench mp2_timings --features bench-support -- --save-baseline before
cargo bench --bench mp2_timings --features bench-support -- --baseline before
```

See the [MP2 benchmark source](https://github.com/mveril/RustiQ/blob/main/benches/mp2_timings.rs)
and [ERI benchmark source](https://github.com/mveril/RustiQ/blob/main/benches/eri_timings.rs).
Benchmarks provide performance measurements, not scientific validation.

### Generate reproducible reports

From the repository root, the scientific report reuses the canonical PySCF
reference cases and their absolute tolerances:

```sh
uv run --locked python tools/reference/report.py --output-dir target/scientific-report
```

It writes full precision results to `validation.json` and readable tables to
`validation.md`. In addition to the cases run by `pytest tools/reference`, it
includes the frozen-core MP2 variants exercised by those tests at 32 KiB and
512 MiB workspace budgets. Total HF energies and MP2 correlation energies are
reported separately. A case fails if either implementation does not converge,
an energy comparison exceeds its existing tolerance, or the UHF spin expectation
value misses its tolerance. Failed cases remain in the report, and the command
returns a nonzero status. The report records software versions, commit and
platform metadata so results can be compared across revisions.

Generate Criterion measurements and their report with:

```sh
cargo bench --features bench-support --bench eri_timings
cargo bench --features bench-support --bench mp2_timings
uv run --locked python tools/benchmarks/report.py
```

The report is written to `target/performance-report/benchmarks.json` and
`benchmarks.md`. To produce both reports on GitHub, open **Actions → Scientific
and performance report → Run workflow** on the selected branch or commit. The
workflow uploads `rustiq-scientific-performance-reports`; download that artifact
from the completed workflow run to inspect the Markdown and JSON files.

Criterion stores structured estimates in `target/criterion`; the benchmark
executables also write the case dimensions and configuration used by the report
generator there. The default runs include the small ERI cases and MP2 AO sizes
12, 24, and 48. To select a smaller local run or opt into the expensive ERI
cases, set `RUSTIQ_BENCH_FILTER`, `RUSTIQ_MP2_SIZES`, `RUSTIQ_MP2_MEMORY`, and
`RUSTIQ_BENCH_HEAVY` as described above. The report includes Criterion means
and confidence intervals. Its configured MP2 workspace is not measured peak
RSS, and synthetic MP2 cases do not model complete molecular calculations.
Results are machine-dependent; the on-demand GitHub Actions workflow starts
with one small ERI case and AO sizes 12 and 24. To run heavy ERI cases, select
**Include the heavy ERI cases** and choose an ERI filter that matches those cases.
The checkbox enables `RUSTIQ_BENCH_HEAVY=1`; it is disabled by default.

These reports provide numerical regression checks for selected cases. They do
not establish general chemical accuracy; see the [scientific scope](../scientific-scope.md)
for the limits of HF, MP2, and numerical agreement. Equal numeric convergence
thresholds do not mean that RustiQ and PySCF use identical convergence tests.

### Independent AO integral validation

The parameterized Cargo tests
`integral_reference_tests::export_integral_validation::case_N` compare
production overlap (S), kinetic (T), nuclear attraction (V), and
Hcore = T + V matrices with versioned PySCF 2.14.0 fixtures. It compares every
matrix entry and every unique unscreened electron-repulsion integral (ERI),
in chemists' notation. No Python installation is needed for Cargo tests.
The cases are H2/STO-3G (2 AOs), H2O/STO-3G (7), H2O/6-31G (13), and
H2O/cc-pVDZ with spherical 5D (24) and Cartesian 6D (25) functions. The
Cartesian cc-pVDZ water case is also rotated by 0.37 radians around the
normalized (1, 2, 3) axis with RustiQ's `Geometry::rotate`; its independently
generated PySCF reference uses the same Rodrigues rotation. This checks the
Cartesian integrals with coordinates spanning all three Cartesian axes.

All families use an absolute tolerance of 1e-10 in each family’s units, with no relative
acceptance criterion. Measured maxima on these cases are below 9e-14; the
threshold leaves margin for floating-point accumulation and platform differences
in contracted Hermite/Boys evaluation while remaining below the 1e-8 scale that
would require investigation. Relative errors are diagnostic only and are reported
for reference magnitudes at least 1e-8. Reports include dimensions, counts, RMS
and maximum absolute error, exceedance counts, and the five largest discrepancies
with zero-based AO indices and full precision values. For S the units are
dimensionless; T, V, Hcore, and ERIs are in atomic units of energy.

The fixtures are generated independently by PySCF/libcint from the same raw
checked-in Basis Set Exchange (BSE) JSON exponents and contraction coefficients,
not from RustiQ's normalized primitives. PySCF interprets and normalizes those
coefficients independently. This avoids differences in rounded packaged basis
sets. Geometry is converted from the checked-in Angstrom XYZ files to Bohr using
0.529177210903 Angstrom/Bohr (CODATA 2018), then supplied explicitly in Bohr.
The JSON metadata records source hashes, PySCF version, AO permutation, units,
layout, and each numerical array hash. Cargo rejects changed source inputs or
array contents; pytest also regenerates every reference array and checks it
within 1e-13 absolute units. Regeneration never reads RustiQ integral values.

RustiQ retains atom, BSE shell, and contraction order; PySCF stably sorts shells
by angular momentum. Within each p shell RustiQ uses z,y,x versus PySCF x,y,z.
The real spherical d order and phases agree: xy, yz, (2zz−xx−yy), xz, (xx−yy),
with normalized polynomial combinations. Cartesian order is reversed:
RustiQ zz,yz,yy,xz,xy,xx versus PySCF xx,xy,xz,yy,yz,zz. The mapping is constructed
from these conventions, never fitted to integral values. PySCF's Cartesian d
functions have radial normalization: multiply its xx,yy,zz functions by
sqrt(5/(4π)) and xy,xz,yz by sqrt(15/(4π)) to obtain unit-normalized monomials.
These factors follow from angular integrals of squared monomials, 4π/5 and
4π/15, respectively. The reference generator checks the resulting unit overlap
diagonals. A separate analytic coefficient test compares the spherical d
polynomials with PySCF’s Cartesian-to-spherical transformation, checking phases
that the planar water geometry alone cannot distinguish. This positive diagonal scaling has condition number sqrt(3); there
is no fitted rotation or phase adjustment. The Cartesian extension changes only
the in-memory shell representation, preserving every source exponent and
coefficient and leaving fixtures and production defaults unchanged.
See the authoritative [PySCF basis construction source](https://pyscf.org/_modules/pyscf/gto/mole.html)
and [integral interface](https://pyscf.org/_modules/pyscf/gto/moleintor.html).

Reproduce the independent comparisons and verify the fixtures:

```sh
cargo test -p rustiq-chem-core --lib integral_reference_tests
nix develop .#mini-pyscf --command pytest tools/reference
# Equivalent non-Nix workflow:
uv run --locked pytest tools/reference
# Intentionally regenerate versioned NumPy arrays and provenance:
uv run --locked python tools/reference/integrals.py --regenerate
# Write the detailed integral-only machine-readable report:
uv run --locked python tools/reference/integrals.py --report target/integrals.json
```

NumPy writes individual `.npy` fixtures; Rust reads them directly
through the existing persistence readers backed by `npyz`. Matrices have
two-dimensional shapes and Fortran (column-major) storage. ERIs use lower triangular AO pairs i≥j, followed by lower
triangular pairs of pairs p≥q, with pair index i(i+1)/2+j. No full four-index
tensor is allocated. The scientific report above includes an independent AO
integral section and full precision JSON diagnostics. Harness failures are
reported as failures, not omitted.

Three forms of evidence remain distinct: independent integral comparisons
check agreement with libcint; internal eightfold compact reconstruction checks
validate storage symmetry; end-to-end HF/MP2 comparisons validate selected
calculations. The compact reconstruction check is algebraic and does not
independently recompute eight permutations. Existing Schwarz screening tests
remain separate; this accuracy test passes `None` to the production threshold
interface and does not change default screening. Successful checks establish
agreement only for these geometries, contractions, s/p functions, and spherical
and Cartesian d functions. They do not establish correctness for higher angular
momenta, extreme exponents, arbitrary geometries, or basis completeness.
