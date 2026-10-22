# Reference Comparisons

This directory contains optional live PySCF comparisons of sample energies and
AO integrals, plus independent integral reference generation. Ordinary Cargo
unit tests compare production integrals with committed NumPy fixtures without
requiring Python or PySCF.

The Nix development shell provides the PySCF version locked in `uv.lock` on all
platforms declared by the flake. Inside the Dev Container, the same tools are
prepared automatically and are available directly on `PATH`. No `.venv` or
activation step is required.

From the Dev Container, run all reference comparisons with the canonical interface:

```sh
pytest tools/reference
```

Outside the Dev Container, the minimal Nix shell includes the same test group:

```sh
nix develop .#mini-pyscf --command pytest tools/reference
```

From another Nix-capable environment, run:

```sh
nix develop --command pytest tools/reference
```

The flake also exposes a manual app that supplies the Nix-built RustiQ binary,
Python, and PySCF without entering the development shell. Run it from the
repository root:

```sh
nix run .#pyscf-check
```

This app is deliberately not part of the flake's `checks`, so `nix flake check`
does not run the PySCF comparisons automatically. PySCF is built from its
binary wheel through `uv2nix`, independently of whether nixpkgs packages it for
the current platform.

Without Nix, install the Rust toolchain described in the root `README.md` and
[`uv`](https://docs.astral.sh/uv/). Then run:

```sh
uv run --locked pytest tools/reference
```

This uses the cross-platform lock for Python 3.12 through 3.14 and requires
compatible binary wheels; it does not build Python packages from source. PySCF
supports Linux, macOS, and WSL2, but the pinned release has no native Windows
wheel. The lock excludes PySCF on native Windows. Windows uv jobs still run the
PySCF-independent report and helper tests; the PySCF comparison and AO
integral suites run in the Nix PySCF environment or WSL2. Cargo must remain on
`PATH` when using the default `cargo run` invocation.
Set `RUSTIQ_BIN` to an absolute executable path to compare a specific build
instead; `nix run .#pyscf-check` supplies its Nix-built executable this way.
The full directory suite also invokes Cargo for Rust/NumPy persistence
interoperability checks, even when `RUSTIQ_BIN` is set.

Select the H₂/STO-3G RHF comparison without its MP2 variant:

```sh
uv run --locked pytest tools/reference -k 'h2-sto-3g-rhf and not mp2'
```

The H₂ matrix also covers the split-valence 6-31G and correlation-consistent
cc-pVDZ basis sets:

```sh
uv run --locked pytest tools/reference -k 'h2-6-31g-rhf or h2-cc-pvdz-rhf'
```

Water exercises a polyatomic, polar geometry and oxygen s/p contractions in all
three basis families, plus spherical d polarization functions with cc-pVDZ:

```sh
uv run --locked pytest tools/reference -k h2o
```

The MP2 comparisons cover RHF H₂/STO-3G, H₂/cc-pVDZ, H₂O/6-31G, and
UHF OH/STO-3G with all occupied orbitals correlated (no frozen core). Each case checks both the HF
total energy and the MP2 correlation energy:

Additional H₂O/6-31G and OH/STO-3G comparisons freeze one occupied orbital and
exercise both 32 KiB and 512 MiB MP2 workspace budgets using temporary runfiles.
The smaller budget forces multiple occupied blocks for water.

```sh
uv run --locked pytest tools/reference -k mp2
```

The open-shell, non-degenerate H₂⁺ UHF reference used by the Rust UHF test can
be reproduced with:

```sh
uv run --locked pytest tools/reference -k h2-plus-sto-3g-uhf
```

The equivalent flake app invocation is:

```sh
nix run .#pyscf-check -- -k h2-plus-sto-3g-uhf
```

Prefix the single-case command with `nix develop --command` when running it
outside the Dev Container or an active Nix development shell.

The versioned store under `tests/data/reference/` contains unmodified basis files
downloaded from Basis Set Exchange with RustiQ. At session startup, pytest copies
the required files into a temporary store, so all comparisons run offline. To
refresh a fixture while keeping the source explicit, point
`RUSTIQ_DATA_BASIS` at `tests/data/reference/RustiQ/basis_sets` and run
`cargo run -- basis download <name>`.
`uv.lock` records the wheels and
hashes used by both uv and Nix; update it intentionally with `uv lock` whenever
the Python dependency declarations change.

Generate a full precision scientific validation report from the same cases and
comparisons:

```sh
uv run --locked python tools/reference/report.py --output-dir target/scientific-report
```

The command writes `validation.json` and `validation.md`. It includes the
canonical cases plus the four frozen-core MP2 cases at each memory budget used
by pytest. A failed or non-finite calculation remains visible in the report and
causes a nonzero exit status. Non-finite diagnostic energies are recorded as
JSON `null`, with their NaN or infinity state retained as a diagnostic string.
The JSON contains machine-readable values;
Markdown formats energies for reading. The comparison only validates selected
numerical cases against PySCF. It does not establish general chemical accuracy
or make HF/MP2 reliable for every system. Matching thresholds also do not imply
identical convergence criteria.

RustiQ is invoked with `run --format json`; pytest reads schema version 1 JSON
directly, checks convergence, methods, and energies, and provides all comparison
reporting. This keeps reference validation independent of terminal report wording
and decimal formatting. `compare_pyscf.py` owns the reference case definitions
and shared comparison functions. Its command-line entry point forwards arguments
to pytest for compatibility; use the commands above for new workflows.

PySCF-independent helper and report tests live in `test_compare_pyscf_helpers.py`
and `test_report.py`. The scientific comparison and AO integral tests require
PySCF and a RustiQ executable. The restricted workspace-budget test and
Rust-backed persistence checks are marked `requires_rust`; Python-only CI can
exclude them with `pytest -m 'not requires_rust'` and run the Rust-backed tests
in an environment that installs the toolchain.

Independent AO integral tests are in `test_integrals.py`; the corresponding
Cargo unit test runs without Python against versioned NumPy fixtures under
`crates/rustiq-core/tests/data/integrals/`. NumPy generates the arrays and Rust
loads them using `npyz`. Regenerate with:

```sh
uv run --locked python tools/reference/integrals.py --regenerate
```

The generator uses only PySCF and raw BSE source data. Pytest verifies the
stored arrays and compares live production Rust integrals against PySCF.
See the manual’s [Independent AO integral validation](../../docs/src/development/contributing.md#independent-ao-integral-validation) section for AO mapping,
normalization factors, tolerances, coverage, and limitations. The existing
scientific report includes the integral metrics.

## Expanded HF/MP2 validation

The canonical `ReferenceCase` matrix adds LiH/6-31G RHF, CO/STO-3G RHF and
RHF-MP2, H₂O/cc-pVDZ RHF-MP2, and OH/6-31G UHF and UHF-MP2. The existing
H₂O/cc-pVDZ RHF case and independent AO fixtures are preserved. Primary MP2
cases correlate every occupied orbital. New samples explicitly set the HF
method, convergence threshold `1e-12`, DIIS history, and iteration limit
(100, or 80 for water). All energies are in Hartree.

Geometry provenance and reuse conditions are maintained in the repository's
[`DATA_SOURCES.md`](../../DATA_SOURCES.md), also included in the manual.
LiH and CO use project-authored, equilibrium-like test distances of 1.60 and
1.13 Å along z; these are deliberately rounded test inputs, not experimental
coordinate exports or optimized structures. Water and OH reuse the existing
files byte-for-byte; neither acquires an external geometry provenance through
this validation. OH remains at 0.970 Å.

| System / basis | Charge / multiplicity | α / β electrons | AOs | α / β virtual orbitals |
|---|---|---|---|---|
| LiH / 6-31G | 0 / 1 | 2 / 2 | 11 | 9 / 9 |
| CO / STO-3G | 0 / 1 | 7 / 7 | 10 | 3 / 3 |
| H₂O / cc-pVDZ | 0 / 1 | 5 / 5 | 24 | 19 / 19 |
| OH / 6-31G | 0 / 2 | 5 / 4 | 11 | 6 / 7 |

The new cases construct PySCF basis shells from the existing raw BSE JSON,
independently of RustiQ's integral and orbital calculations. Both engines
normalize the raw primitives/contractions themselves. The versioned 6-31G file
already contains lithium and STO-3G already contains carbon, so no fixture is
changed or downloaded. Spherical functions are used, including oxygen's five
d functions in cc-pVDZ. Coordinates are independently converted to Bohr with
`1 Bohr = 0.529177210903 Å` to match RustiQ's unit convention. Existing
packaged-basis reference cases retain their original tolerances and conventions.

Each new RustiQ execution writes a temporary portable artifact. Its resolved
configuration and Bohr XYZ are inspected and checked against the case metadata,
including method, charge, multiplicity, basis name, coordinates, SCF settings,
and frozen-orbital count. The OH random guess and seed are also checked.
Electron counts are inferred from nuclear charges, molecular charge and
multiplicity and checked against PySCF's actual counts; the public result schema
does not expose RustiQ electron counts. Full AO rank is required by the expected
occupied/virtual partition. Store and cache overrides are isolated in a temporary
directory; the report retains resolved snapshots and basis/geometry hashes.

### Convergence and the OH electronic state

PySCF uses `conv_tol=1e-12`, `conv_tol_grad=1e-10` and a core-Hamiltonian (`1e`)
guess for the new cases. Its default gradient threshold would be
`sqrt(conv_tol)=1e-6`; an initial CO trial passed the strict HF comparison but
left an MP2 correlation difference around `2e-9` Ha. Tightening the orbital
gradient reduced that discrepancy to the values below without loosening the
energy comparison. The codes' stopping criteria remain different.

For OH/6-31G, RustiQ's unperturbed core-Hamiltonian guess found a converged
higher-energy state: HF `-75.20799697500071`, correlation
`-0.08669669368363855`, and HF ⟨S²⟩ `0.7567946224284676`. Independent PySCF
C₂ᵥ occupations identify this as a sigma-hole branch: `A1=(3,2)`,
`B1=(1,1)`, `B2=(1,1)` (alpha, beta counts per irrep). PySCF reproduces that
branch's HF energy within `1e-13` Ha and correlation within `2e-12` Ha.
It is a state-selection discrepancy, not evidence of an integral defect.

The new OH samples explicitly use the existing normal random-density guess
(mean 0, standard deviation 1, seed 42). Seeds 1, 7 and 42 independently reached
the lower-energy branch with matching energies and spin diagnostics. PySCF
core, MINAO and atomic guesses agree on that branch; its C₂ᵥ occupation is
`A1=(3,3)`, `B1=(1,1)`, `B2=(1,0)`, a pi hole. This supports comparison of
equivalent solutions; it does not establish global HF stability. The reported
spin contamination is ⟨S²⟩ − 0.75 for these doublets and belongs to the HF
reference, not a correlated MP2 spin expectation.

### Absolute tolerances

Measured errors with pinned PySCF 2.14.0 and single-thread BLAS/OpenMP are below.
The tolerances leave a numerical margin over the observed errors for independent
SCF termination and platform-dependent floating-point reductions; they do not
represent physical error bars. No existing tolerance is increased.

| System / basis | HF error / tolerance (Ha) | MP2 correlation error / tolerance (Ha) | ⟨S²⟩ error / tolerance |
|---|---|---|---|
| LiH / 6-31G | 1.07e-14 / 1e-11 | — | — |
| CO / STO-3G | 6.25e-13 / 2e-11 | 5.98e-13 / 2e-11 | — |
| H₂O / cc-pVDZ | 2.84e-14 / 1e-11 | 3.95e-13 / 1e-11 | — |
| OH / 6-31G | 2.84e-14 / 2e-11 | 1.73e-12 / 2e-11 | 2.44e-11 / 2e-9 |

LiH's margin covers small SCF/reduction differences; CO receives a larger
margin because its observed HF and correlation differences are larger. Water's
strict margin covers the independently validated spherical-d transformation.
OH's larger correlation and spin margins cover sensitivity to the converged
open-shell orbitals and the observed guess variations. All comparisons use
absolute tolerances and zero relative tolerance. `validation.json` and
`validation.md` retain full floating-point precision rather than this rounded
summary. MP2 total energy is derived as HF total plus correlation, with the
sum of the two tolerances; the JSON total must also match RustiQ's own sum
within `1e-12` Ha. Electronic-energy consistency is checked separately by pytest.

Frozen-one-orbital CO and OH variants extend the existing 32 KiB/512 MiB
workspace checks. Text-mode planner observations verify actual occupied block
sizes: CO uses 2 versus 6; OH alpha-alpha/alpha-beta use 2 versus 4 and
beta-beta uses 2 versus 3. Each constrained execution therefore uses multiple
blocks. Both budgets are independently checked against PySCF and their RustiQ
correlation energies agree within `1e-12` Ha. Freezing removes one spatial
occupied orbital for RHF and one occupied orbital from each UHF spin channel;
virtual spaces remain unchanged. These budgets govern transformation workspace,
not complete process memory or total AO ERI storage.

For controlled reproduction, prefix either the Nix or uv command above with:

```sh
OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 RAYON_NUM_THREADS=2
```

The scientific report automatically includes the new canonical and frozen
cases. Available iteration counts, input-derived electron counts, PySCF virtual
counts, spin contamination, derived MP2 totals, and provenance are additional
JSON fields; failed calculations remain present. Native Windows restrictions
for the pinned PySCF remain unchanged. This extension does not broaden chemical
accuracy claims beyond the listed states, basis definitions, and geometries.
