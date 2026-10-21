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
       -> iterative SCF and final canonicalization
       -> HfOutcome::Converged or HfOutcome::Unconverged
    -> converged HfSolution::mp2()
       blocked AO-to-MO integral transformation and MP2 correlation-energy evaluation
```

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
