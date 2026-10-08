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

- `rustiq-core` owns scientific domain types, calculation preparation,
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
- sequential Nickel batch execution with per-calculation results and errors;
- XYZ geometries with source-located diagnostics;
- local and optional online basis-set management;
- Gaussian basis construction;
- RHF and UHF with DIIS;
- RHF-MP2 and UHF-MP2;
- blocked AO-to-MO MP2 transformations with explicit memory planning;
- geometry inspection and transformations;
- structured calculation preparation through `CalculationRequest` and
  `PreparedCalculation`;
- local directory-backed AO ERI reuse through typed persistence primitives;
- portable V1 multi-calculation containers with integrity checks and atomic
  publication (#88);
- versioned machine-readable calculation output;
- unit, integration, sample, and PySCF reference tests;
- benchmark support for important ERI and MP2 paths.

The next development cycle is primarily an architecture-stabilization cycle.
Large new scientific methods should not outrun the configuration, persistence,
reuse, validation, and public-API boundaries described below.

## Architecture stabilization sequence

The main architectural work should proceed in the following order:

```text
portable persistence V1
        |
        v
frontend-neutral resolved configuration
        |
        v
Nickel configuration, batch resolution, and CLI batch execution
        |
        v
portable-artifact CLI orchestration
        |
        v
HF solution and restart persistence
        |
        v
scientific validation / stable public APIs
```

Some independent validation, documentation, and performance work may proceed in
parallel when it does not change these boundaries.

### 1. Portable persistence V1 — delivered (#79, #88)

Delivered by #88: a portable, versioned ZIP/ZIP64 `.rustiq` container with
integrity checks, snapshot validation, atomic publication, interoperability
coverage, and a natively multi-calculation V1 manifest. Source provenance,
requested/resolved state, and artifacts remain distinct behind typed APIs.
Normal CLI artifact orchestration remains a downstream milestone under #79.

#### Portable manifest is multi-calculation from V1

A portable `.rustiq` represents a **non-empty** collection of one or more
calculations (`1..N`). A normal single calculation is therefore the common
case where the collection contains exactly one entry.

This is a container-level capability only. It must not introduce a scientific
`BatchConfig`, `SingleOrBatch`, or other batch abstraction into
`rustiq-core` calculation APIs.

The conceptual shape is:

```text
portable .rustiq
├── format / producer
├── shared source provenance
└── calculations
    ├── calculation 0
    │   ├── normalized request snapshot
    │   ├── resolved calculation snapshot
    │   └── typed scientific artifacts
    ├── calculation 1
    │   └── ...
    └── ...
```

The manifest must not assign one fake scientific identity to the whole batch.
Scientific compatibility remains local to the calculation and, where
appropriate, to the individual artifact representation.

Consequences:

- `calculations.len() == 1` is the ordinary non-batch case;
- a Nickel input resolving several calculations can be preserved in one
  `.rustiq` without changing the container format;
- batch-level inputs such as one Nickel source may be stored once as shared
  opaque provenance;
- each calculation has its own request/resolved snapshots and typed artifact
  references;
- compatibility and reuse are evaluated independently for each calculation and
  artifact;
- one incompatible calculation does not invalidate scientifically independent
  entries in the same portable bundle;
- artifact payload deduplication between calculations is optional and can be
  added later without changing the semantic model;
- the machine-local cache remains calculation/artifact-oriented and does not
  need to become a batch container.

The wire layout and stable calculation identifiers are defined by #88 under
#79. Source ordering, user-facing labels, and aggregate batch hashes must not
become part of scientific compatibility.

### 2. Canonical Nickel configuration frontend — delivered (#94)

The exploratory POC in #96 has validated the key frontend decisions before the
production migration starts:

- real RustiQ TOML inputs can be normalized through Nickel contracts/defaults;
- tagged guess/distribution variants can remain closed and preserve current
  semantics;
- one calculation and Nickel-generated batches normalize to a non-empty
  `ResolvedInput { calculations: Vec<_> }` boundary;
- `nickel-lang` can evaluate the configuration in-process, so users do not need
  an external Nickel executable;
- Nickel diagnostics can be adapted to RustiQ's source-aware reporting.

The POC also exposed a Nickel 2.2 debug-assertion bug when applying nested record
contracts to imported/deserialized data. #94 must track an upstream-safe
resolution or keep any temporary workaround private to the frontend; the
workaround must not become part of RustiQ's public configuration contract.

The production frontend boundary now builds on the merged portable V1 shape.

The production flow is:

```text
.toml ----\
          +--> frontend resolution --> ResolvedInput
.ncl  ----/                         Vec<ResolvedCalculationConfig>
                                            |
                                            v
                                      rustiq-core
```

Delivered incrementally:

1. #99 established the sample configuration compatibility baseline.
2. #100 introduced resolved DTOs, fallible core conversion, and maintained
   Nickel schema verification.
3. #101 made Nickel authoritative for TOML, preserved source mapping and
   grouped diagnostics, migrated canonical TOML and `init`, and removed direct
   `toml` and `toml-spanner` dependencies.
4. Native `.ncl` inputs now support imports, one record or a non-empty array,
   full batch resolution, and sequential execution of every calculation.

Batch execution continues after individual preparation/execution failures.
JSON batches report ordered successes, non-convergence, and errors; individual
calculations retain the existing JSON V1 contract. Imports use Nickel semantics,
while geometry paths are relative to the top-level input. Computed scientific
values have no fabricated source spans.

This completes the frontend responsibility of #94 and delivers basic CLI batch
orchestration. Persistence/reuse integration and HF restart remain under #79.

Nickel owns user-facing structure, defaults, composition, and configuration
validation. `rustiq-core` continues to enforce scientific invariants for
direct Rust API consumers.

### 3. Integrate portable artifacts into execution and the CLI — next (#79)

After the frontend boundary is stable, introduce portable reuse into the normal
execution planner instead of bolting archive handling onto the current
TOML-specific run path.

The intended precedence for each required artifact is:

```text
explicit portable .rustiq
        |
        v
enabled local cache
        |
        v
computation
```

The CLI milestone includes:

- `rustiq run ... --artifact result.rustiq`;
- `rustiq run ... --reuse previous.rustiq`;
- read-only reuse by default;
- transactional same-path completion;
- `rustiq artifact inspect`;
- explicit reporting of reused, missing, incompatible, ignored, and recomputed
  state;
- artifact/result orchestration for every calculation in an already executable
  Nickel batch, with independent compatibility and reuse decisions.

Batch orchestration should consume `ResolvedInput`; it must not leak batch
concepts into individual scientific calculation APIs.

### 4. Persist converged and restartable HF state (#79)

Once basic portable reuse works end to end, extend typed persistence beyond AO
ERIs.

Implement in this order:

1. converged RHF/UHF solutions reusable by post-HF methods;
2. partial RHF/UHF restart state without DIIS;
3. optional versioned DIIS continuation state.

A compatible MP2 request should be able to reuse a converged HF solution
without rerunning SCF. A compatible interrupted HF calculation should be able
to continue from persisted partial state.

Restart semantics must be scientifically defined and reported explicitly.
Persistence must never silently claim exact continuation when required state is
missing or incompatible.

### 5. Strengthen scientific validation and reproducibility

Architecture alone is not enough for research use. Once the main input and
persistence contracts stabilize, prioritize:

- systematic RHF, UHF, RHF-MP2, and UHF-MP2 comparison against established
  implementations;
- documented numerical conventions for units, Gaussian normalization, ERI
  ordering/symmetry, SCF, spin treatment, and MP2;
- documented tolerances and versioned reference datasets;
- non-regression coverage across molecules, basis sets, charge states, spin
  states, and difficult SCF cases;
- reproducible ERI, SCF, and MP2 benchmarks;
- larger-calculation memory and performance characterization.

This work may add bugs or performance issues to the roadmap, but it should not
destabilize the frontend/persistence boundaries without a concrete scientific
reason.

### 6. Stabilize embedding APIs and bindings

Bindings should target the stabilized core API rather than duplicate CLI
configuration behavior.

After the architecture work above:

- refresh the Python bindings against the current `rustiq-core` public API;
- expose domain-native molecule, geometry, basis, HF, and MP2 operations;
- keep Python configuration ergonomic without mirroring the Nickel/TOML
  frontend internally;
- evaluate C bindings only where a concrete embedding use case exists;
- document API stability expectations before expanding the binding surface.

### 7. Packaging and release readiness

Once the scientific and architectural contracts are sufficiently stable:

- produce release artifacts for common Linux, macOS, and Windows targets;
- validate installation without the development Nix environment;
- keep Nix/devcontainer workflows reproducible for contributors;
- publish clear compatibility and migration notes for persistent formats and
  public APIs.

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
