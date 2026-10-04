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

- TOML runfiles and XYZ geometries with source-located diagnostics;
- local and optional online basis-set management;
- Gaussian basis construction;
- RHF and UHF with DIIS;
- RHF-MP2 and UHF-MP2;
- blocked AO-to-MO MP2 transformations with explicit memory planning;
- geometry inspection and transformations;
- structured calculation preparation through `CalculationRequest` and
  `PreparedCalculation`;
- local directory-backed AO ERI reuse through typed persistence primitives;
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
Nickel configuration and batch resolution
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

### 1. Finalize portable persistence V1 (#79, #88)

The immediate persistence milestone is a portable, versioned `.rustiq`
container built on the typed persistence work already present in
`rustiq-core`.

Before the portable format is considered stable:

- finish the ZIP/ZIP64 container, integrity checks, snapshot validation, atomic
  publication, and Rust/Python interoperability work;
- keep source provenance, normalized requested state, resolved scientific
  state, and numerical artifacts distinct;
- keep physical ZIP details behind typed persistence APIs;
- fix the remaining CI failures in #88;
- make the portable V1 manifest natively multi-calculation before publishing it
  as a durable contract.

#### Portable manifest is multi-calculation from V1

A portable `.rustiq` represents a collection of one or more calculations.
A normal single calculation is therefore the common case where the collection
contains exactly one entry.

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

The exact wire layout and stable calculation identifiers belong to #79/#88.
They should be deterministic and should not make source ordering, user-facing
labels, or an aggregate batch hash part of scientific compatibility.

### 2. Make Nickel the canonical configuration frontend (#94)

Once the portable storage boundary is clear, stabilize the frontend boundary.

The target flow is:

```text
.toml ----\
          +--> frontend resolution --> ResolvedInput
.ncl  ----/                         Vec<ResolvedCalculationConfig>
                                            |
                                            v
                                      rustiq-core
```

Implement #94 incrementally:

1. characterize the current TOML behavior with regression tests;
2. introduce frontend-neutral fully resolved DTOs;
3. implement Nickel contracts/defaults/validation in shadow mode;
4. switch TOML resolution to the Nickel-backed schema;
5. add native `.ncl` support;
6. normalize both one-calculation and multi-calculation inputs to
   `ResolvedInput { calculations: Vec<_> }`;
7. separate source mapping from configuration validation;
8. migrate canonical TOML generation and `rustiq init`;
9. remove `toml-spanner` once it has no remaining responsibility.

Nickel owns user-facing structure, defaults, composition, and configuration
validation. `rustiq-core` continues to enforce scientific invariants for
direct Rust API consumers.

### 3. Integrate portable artifacts into execution and the CLI (#79)

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
- execution of every calculation in a resolved Nickel batch without silently
  dropping entries.

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
