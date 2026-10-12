# Roadmap

RustiQ aims to demonstrate that quantum-chemistry software can be modern,
cross-platform, typed, tested, inspectable, and pleasant to extend without
sacrificing scientific correctness or architectural clarity.

This document is the project-level roadmap. Detailed requirements, acceptance
criteria, and implementation notes remain in their dedicated GitHub issues and
pull requests.

## Architectural principles

The roadmap follows a few boundaries that should remain stable as the project
grows:

- `rustiq-chem-core` owns scientific domain types, calculation preparation,
  execution, and scientific invariants. It must not depend on TOML, Nickel,
  terminal presentation, or application-directory policy.
- User-facing configuration is normalized before entering the scientific
  pipeline. A frontend may resolve one calculation or several calculations, but
  the core still operates on individual scientific calculations.
- TOML remains a readable single-calculation format. Nickel is the programmable
  configuration layer for composition and batch generation.
- The active local cache and portable `.rustiq` files are separate products:
  the cache is disposable machine-local execution policy, while `.rustiq` is a
  durable scientific artifact.
- Portable persistence is snapshot-oriented and typed. Numerical code consumes
  Rust domain objects rather than arbitrary archive paths or mutable key/value
  state.
- Original input files are optional opaque provenance. They are useful for
  inspection and diagnostics but never define scientific identity.
- Reuse compatibility is evaluated per scientific artifact and per calculation.
  A portable bundle must not become all-or-nothing merely because several
  artifacts or several calculations are stored together.
- Publication of portable state is transactional: a complete validated snapshot
  is written before it becomes visible at its destination.

## Current baseline

RustiQ currently provides:

- TOML and native Nickel inputs with embedded contracts/defaults, composition,
  imports, and source-aware configuration diagnostics;
- sequential Nickel and portable-bundle batch execution with ordered
  per-calculation results and errors;
- XYZ geometries, geometry inspection and transformations;
- local and optional online basis-set management, with Cartesian and supported
  spherical Gaussian basis functions;
- RHF and UHF with optional DIIS, plus RHF-MP2 and UHF-MP2;
- blocked AO-to-MO MP2 transformations with explicit workspace planning;
- reusable scientific input preparation through `CalculationRequest` and
  `PreparedCalculation`;
- optional directory-backed local AO ERI reuse;
- portable V1 multi-calculation `.rustiq` bundles with typed request/resolved
  snapshots, integrity checks, and atomic publication;
- CLI bundle creation, read-only reuse, inspection, payload verification,
  explicit same-path replacement, and direct archive reruns;
- versioned calculation, batch, and artifact-inspection JSON contracts;
- unit, integration, sample, persistence-interoperability, and PySCF reference
  checks with explicit case-specific tolerances;
- Criterion ERI/MP2 benchmarks and reproducible scientific/performance reports;
- binary release and packaging-smoke workflows for six Linux, macOS, and
  Windows targets, plus a separate crates.io publishing workflow;
- an mdBook manual and Cargo, Nix, and Dev Container contributor workflows.

These capabilities remain experimental. In particular, portable bundles store
scientific inputs and AO ERIs, not HF solutions, partial SCF state, DIIS history,
or MP2 results. Direct archive execution starts a fresh HF calculation.

The [manual](docs/src/SUMMARY.md), [CLI reference](docs/src/reference/cli.md),
[schemas](schemas), and [persistence specification](docs/persistence-format-v1.md)
describe implemented behavior. Issue and pull-request descriptions may retain
historical proposals; shipped code, contracts, and tests establish availability.

## Delivered architecture milestones

### Portable persistence V1 — delivered (#88)

The non-empty, multi-calculation ZIP/ZIP64 container, typed snapshots, AO ERI
artifacts, interoperability checks, and atomic publication are implemented.
Each calculation has its own scientific state and artifact compatibility;
source order and shared input provenance do not define a batch scientific
identity. A single calculation is the ordinary one-entry container case.

The [persistence specification](docs/persistence-format-v1.md) owns the wire
layout, calculation identifiers, versioning, validation, and Rust APIs. Future
artifact representations should extend those contracts rather than duplicate
them in the roadmap. The local disposable cache remains separate from a
portable scientific snapshot.

### Canonical Nickel configuration frontend — delivered (#94)

The migration was delivered by #99, #100, #101, and #103. TOML and native `.ncl`
inputs share Nickel defaults, contracts, and normalization into a non-empty
`ResolvedInput` collection. Nickel evaluates in-process; no external Nickel
executable is needed. The CLI executes calculations sequentially, retaining
ordered per-entry outcomes after recoverable failures.

```text
.toml ----\
          +--> Nickel resolution --> ResolvedInput
.ncl  ----/                         Vec<ResolvedCalculationConfig>
                                            |
                                            v
                              individual rustiq-core calculations
```

Imports follow Nickel semantics; geometry paths resolve relative to the
top-level input. Nickel owns frontend structure and defaults, while the core
also enforces scientific invariants for direct API consumers.

The nested-record contract workaround discovered during the migration remains
private to the frontend. Its current rationale and removal conditions are
maintained in the [Nickel schema README](src/runfile/nickel/README.md).

### Portable artifacts in execution and the CLI — delivered (#108)

The normal TOML/Nickel execution path now selects compatible AO ERIs in this
order:

```text
explicit --reuse .rustiq
        |
        v
enabled local cache
        |
        v
computation
```

The CLI supports `run INPUT --artifact result.rustiq`,
`run INPUT --reuse previous.rustiq`, `artifact inspect`, `artifact verify`, and
`run previous.rustiq`. Explicit same-path reuse/output publishes a replacement
snapshot transactionally. Compatibility decisions remain independent for each
calculation in a Nickel batch; output represents the current requested
collection rather than accumulated history.

Direct bundle reruns reconstruct archived resolved inputs and basis data,
without requiring the original runfile, geometry file, or local basis store.
They run fresh HF/MP2 calculations, reusing compatible stored AO ERIs when
present and computing them otherwise. The [portable bundle guide](docs/src/user-guide/portable-bundles.md)
owns the runnable workflows and their restrictions.

### Validation, performance, documentation, and release infrastructure — delivered baseline

Selected RHF, UHF, RHF-MP2, and UHF-MP2 cases are compared against pinned PySCF.
Frozen-orbital and MP2 workspace variants are included, and report generation
records full precision results, tolerances, versions, and execution metadata
(#121). Criterion benchmarks cover ERI and MP2 operations (#117), including a
guaranteed multiblock MP2 workload (#125).
See [reference comparisons](tools/reference/README.md) and
[performance reporting](docs/src/development/contributing.md#generate-reproducible-reports).

Binary release workflows (#119) and native packaging rehearsals (#120, #122,
#123) cover Linux, macOS, and Windows on x86_64 and ARM64. They provide release
infrastructure and packaging checks; availability of published versions is
shown on [GitHub Releases](https://github.com/mveril/RustiQ/releases).
The [release guide](docs/src/development/releasing.md) describes tag validation,
checksums, smoke verification, and separate crates.io publication.

The manual covers binary and source workflows, scientific limits, input/output
contracts, persistence, development, and verification (#113, #115, #124).
Existing validation and packaging checks establish a baseline for further work,
not research-grade scientific or universal platform guarantees.

## Remaining priorities

### 1. Persist converged and restartable HF state (#79)

Portable AO ERI reuse is implemented. The next persistence milestone extends
typed artifacts beyond integrals, in this order:

1. converged RHF/UHF solutions reusable by post-HF methods;
2. partial RHF/UHF restart state without DIIS;
3. optional versioned DIIS continuation state.

A compatible MP2 request should be able to reuse a converged HF solution
without rerunning SCF. A compatible interrupted HF calculation should be able
to continue from persisted partial state.

Restart semantics must be scientifically defined and reported explicitly.
Persistence must never silently claim exact continuation when required state is
missing or incompatible. Advanced CLI artifact-selection/ignore filters and
optional pruning remain follow-up work after basic HF persistence is stable.

### 2. Broaden scientific validation and reproducibility

Build on the existing reference suite and reports by prioritizing:

- broader RHF/UHF/MP2 comparisons across elements, basis conventions, charges,
  spin states, and difficult SCF cases;
- detailed numerical conventions for Gaussian normalization, integral
  ordering/symmetry, SCF, spin treatment, and MP2;
- additional versioned reference cases with explicit tolerances and provenance;
- reproducible SCF performance measurements alongside the existing ERI/MP2
  benchmarks;
- measured larger-calculation memory and performance characterization.

The [scientific scope](docs/src/scientific-scope.md) states the current coverage
and limits. Validation can reveal bugs or performance issues requiring targeted
implementation changes. Large new methods should not outrun the configuration,
persistence, resource, and public-API boundaries.

### 3. Stabilize embedding APIs and evaluate bindings

The reusable `rustiq-chem-core` Rust API and runnable examples already exist.
The current workspace contains the CLI and core crates; the Python tooling is
for reference validation and reporting, not a shipped Python binding package.

Future embedding work should:

- document API stability and migration expectations;
- develop Python bindings against the stabilized scientific API when there is
  a concrete use case;
- expose domain-native molecule, geometry, basis, HF, and MP2 operations;
- keep binding configuration ergonomic without duplicating the Nickel/TOML
  frontend internally;
- evaluate C bindings where a concrete embedding need exists.

### 4. Exercise release readiness and compatibility

The release workflows are implemented. Continue to:

- rehearse all configured native target archives and exercise published
  installation workflows;
- expand compatibility evidence beyond the GitHub-hosted runner environments;
- keep Cargo, Nix, and Dev Container workflows reproducible for contributors;
- publish compatibility and migration notes for persistent formats and public
  APIs;
- update release authentication when crates.io Trusted Publishing prerequisites
  are met, as described in the release guide.

Scientific validation, performance, documentation, and packaging work can
proceed alongside HF persistence when they preserve these boundaries.

## Parallel improvement tracks

The following work may progress alongside the architecture sequence when it
does not force premature abstractions:

- integral screening and batching improvements;
- SCF robustness work;
- memory-use reductions;
- profiling and release-build improvements;
- documentation and examples;
- reference-energy expansion;
- CLI accessibility and presentation improvements.

## Deferred architecture

### Modular online HTTP transport (#59)

Reqwest remains the current online basis-set transport. Do not introduce a
transport abstraction until a real consumer needs runtime independence,
multiple transports, or a non-Tokio implementation.

### Larger scientific scope

The following remain interesting future directions but are not immediate
architecture priorities:

- production DFT;
- geometry optimization and frequencies;
- larger correlated methods;
- GPU acceleration.

## Research-grade requirements

RustiQ should not be described as research-grade until it has:

- systematic validation against trusted reference implementations;
- documented tolerances and reference datasets;
- clear scientific convention documentation;
- reproducible performance benchmarks;
- stable input, persistence, and output contracts;
- defined restart/reuse semantics;
- broader method and basis-set coverage;
- release and interoperability testing across supported platforms.
