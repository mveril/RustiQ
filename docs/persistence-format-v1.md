# RustiQ logical persistence format V1

This document specifies the storage-independent logical format used by RustiQ
scientific persistence, including the portable `.rustiq` ZIP/ZIP64 container.

## Active AO ERI cache

The active deterministic-integral cache is directory-backed and disabled by
default. A calculation opts into reuse explicitly in its runfile:

```toml
[cache]
enabled = true
```

Cache activation is part of the calculation configuration, while the cache root
is a machine-local application choice. The scientific core never chooses a user
or system cache directory. When caching is enabled, the RustiQ CLI uses
`dirs::cache_dir()/RustiQ` by default, `RUSTIQ_CACHE_HOME` when set, or
`rustiq run --cache-dir DIR` for a one-execution location override.
`--cache-dir` does not enable caching by itself. An AO ERI entry has this
layout:

```text
<cache-root>/eri/<scientific-identity-digest>/
├── manifest.json
└── arrays/integrals/ao-eri.npy
```

The cache validates the manifest version, scientific identity, artifact
representation, typed AO ERI attributes, payload size, SHA-256 digest and NPY
header before reading NPY data. AO ERI attributes include the basis-function
count and the current AO ERI computation version. `rustiq cache list` reports entries as `verified` only after the
payload has been checked for the expected byte size, SHA-256 digest, supported
f64 dtype and endianness, one-dimensional rank, and expected value count. These
checks use bounded memory; listing does not construct or load the ERI values.
Any missing, malformed, stale, truncated or corrupted entry is a cache miss and
must be recomputed; it is never used as scientific input. Entries are written
to a sibling temporary directory, finalized and synced, then atomically renamed
into place. Published entries are immutable, so concurrent producers can safely
leave the first completed entry in place.

Use `rustiq cache list` to inspect published entries in a table with `NAME`,
`FINGERPRINT`, `SIZE` and `STATUS` columns. Sizes are human-readable (`unknown`
when unavailable), and statuses are `verified` or `invalid`. An empty cache
prints `No cache entries found.`

Entries receive persistent English aliases such as `calm-photon` or
`quiet-xenon`, generated with `petname`. Nouns include scientific terms (including
`quanta`) and all 118 element names from `periodic_table`. Existing names remain
unchanged when the vocabulary changes. Names are management metadata, not part
of scientific identity, the manifest, or NPY:

```text
<cache-root>/
├── eri/<fingerprint>/manifest.json
└── names/calm-photon -> ../eri/<fingerprint>
```

Unix uses relative symbolic links. Other platforms, or filesystems without
symlink support, use a text file containing the full lowercase fingerprint and
a newline. Readers accept both representations and validate link targets without
following them. Names are atomically reserved without overwriting existing aliases;
collisions try another two-word name, up to 256 attempts. Failure to assign an
alias never invalidates a calculation or its cached integrals.

During `rustiq run`, successful cache publication is reported as
`AO ERI cache: stored as <name>` and reuse is reported as
`AO ERI cache: hit <name>`. If alias metadata cannot be created or read, RustiQ
reports the full 64-character fingerprint instead. Aliases are management-only
metadata and are not part of the scientific result or cache identity.

Names are assigned after publication and, for older entries, by `cache list`.
Listing therefore may create management metadata. Read-only caches remain
inspectable, with `-` for entries without an alias. The core `entries()` API is
read-only; `assign_missing_names()` performs assignment separately. Concurrent
writers may leave several aliases for one fingerprint; all work, and listing
uses the lexicographically first one.

Delete one entry with `rustiq cache remove calm-photon` or
`rustiq cache remove <fingerprint>`. Fingerprints must contain exactly 64 lowercase
hexadecimal characters. Alias resolution does not read or hash NPY data.
Successful removal also deletes associated aliases. `rustiq cache remove --all`
removes published AO ERI entries and recognized orphan aliases without inspecting
payloads. Orphan aliases are not silently reassigned; malformed metadata is left
untouched. Names and fingerprints are validated rather than interpreted as paths.

## Logical entries

The core Rust API exposes `persistence::RustiQData` as a typed scientific
facade. Physical storage selection is an internal persistence concern: the
directory-backed ERI cache resolves folder storage internally, and the
portable `.rustiq` API resolves ZIP/ZIP64 internally rather than exposing a
generic storage backend. Internal readers load the bounded manifest first and
open scientific artifacts on demand. `read_eri` validates and decodes the AO ERI
NPY on first access and keeps the resulting `CompactEri` for subsequent accesses.
Unknown representations can be copied internally as verified byte streams without
being decoded.

The public scientific API is typed: `set_eri` and `read_eri` operate on
`CompactEri`. The generic `get::<AoEriArtifact>()` returns
`Result<Option<&CompactEri>, ArtifactError>`, while
`set::<AoEriArtifact>(value)` accepts only `CompactEri`; only declared artifact
marker types are accepted. Artifact access reports `ArtifactError`, while NPY
format failures are represented by `NpyError`; storage, manifest, and persistence
orchestration errors remain internal. A future known artifact gets its own marker,
typed field, and accessors in `RustiQData`. NPY parsing and conversion remain
internal persistence details. Compact ERIs are decoded only with the
basis-function count from their typed manifest attributes; the NPY length is never
used to infer that scientific context. Matrix readers accept C and Fortran order
and matrix writers emit Fortran order.

For the directory cache:

- `manifest.json` is UTF-8 JSON and describes the format, producer, scientific
  identity and artifacts.
- `arrays/integrals/ao-eri.npy` is the AO electron-repulsion integral artifact.

Artifact paths are portable relative UTF-8 paths with `/` as the only separator,
independent of the host operating system. Empty components, `.`, `..`,
backslashes, drive-like prefixes containing `:`, absolute paths, Windows-reserved
device names, Windows-invalid filename characters, trailing dots/spaces, and paths
conflicting with `manifest.json` are rejected before storage access. Logical paths
are also checked case-insensitively to avoid cross-platform collisions.

Every artifact records common envelope metadata:

- logical path;
- byte size;
- representation identifier;
- content digest in the form `sha256:<lowercase hex>`;
- a representation-specific `attributes` object.

The common artifact envelope deliberately does not contain AO-ERI-specific fields.
Known representations decode their `attributes` into strict typed metadata.
Malformed attributes for a known representation are rejected rather than treated
as an unknown representation. Truly unknown representations preserve their raw
JSON attributes so newer manifests remain inspectable by older readers.
Preserving unknown metadata does not make an unsupported scientific
representation usable: consumers must reject artifacts they do not understand
when those artifacts are required for a calculation.

For `rustiq-compact-eri-v1`, the attributes are:

```json
{
  "basis_functions": 114,
  "computation_version": 1
}
```

NPY-specific metadata such as dtype, endianness and shape remains authoritative
in the NPY header rather than being duplicated in the manifest. Readers validate
the representation-specific semantic attributes together with the actual NPY
metadata, declared size and digest before scientific use. Payload integrity is
independent of its container.

## Generic artifact manifest

The cache root manifest, or each portable calculation entry, indexes scientific artifacts rather than a
union of every scientific state RustiQ may ever persist. A representative entry
has this shape:

```json
{
  "path": "arrays/integrals/ao-eri.npy",
  "size": 123456,
  "representation": "rustiq-compact-eri-v1",
  "digest": "sha256:...",
  "attributes": {
    "basis_functions": 114,
    "computation_version": 1
  }
}
```

The `representation` field selects the semantic contract for both the payload
and its attributes. RustiQ readers use typed attributes for known
representations and retain unknown attributes unchanged for forward-compatible
inspection. Future matrix, SCF restart, converged-HF, and post-HF representations
can therefore define their own attributes without changing the common artifact
envelope.

## `rustiq-compact-eri-v1`

AO ERIs are a one-dimensional NPY array of IEEE-754 binary64 values. Object and
pickled arrays are forbidden. Readers must validate the declared byte size,
dtype, rank, element count and digest before scientific use. Both little- and big-endian binary64 NPY
payloads are readable; writers use the native binary64 dtype emitted by `npyz`,
and the dtype stored in the NPY header defines the payload endianness.

For `n` basis functions, define the symmetric pair index

```text
pair(i, j) = a(a + 1)/2 + b, where a = max(i, j), b = min(i, j).
```

Then define

```text
eri(mu, nu, lambda, sigma) = pair(p, q),
p = pair(mu, nu), q = pair(lambda, sigma).
```

The NPY value at that final index stores `(mu nu | lambda sigma)`. Its length is
`m(m + 1)/2`, where `m = n(n + 1)/2`. This is the stable ordering implemented by
`PairIndex` and `EriIndex`; the NPY shape alone never defines these semantics.

## `scientific-identity-v1`

The identity is SHA-256 over an explicit canonical byte stream, not serialized
Rust structs or JSON. Collections and strings are prefixed by unsigned 64-bit
big-endian lengths; integers are big-endian; floating-point values are their
IEEE-754 binary64 bits in big-endian order, with negative zero normalized to
positive zero.

The stream contains, in order:

1. identity identifier, AO ERI computation version, and ERI representation identifier;
2. ordered atoms: atomic number and coordinates in Bohr;
3. ordered effective AO data consumed by the ERI engine: shell center,
   normalized component angular momentum, primitive exponents and effective
   normalized coefficients;
4. Schwarz screening state, encoded distinctly as disabled or enabled with a
   positive finite threshold.

Producer version, paths, timestamps, compression and container metadata do not
participate in scientific identity.

The AO ERI computation has its own explicit version, independent of the
persistence identity version, NPY representation, and RustiQ package version.
It is included in the canonical scientific-identity digest and is also stored
as the AO ERI artifact attribute `attributes.computation_version`. Bumping that
version changes the fingerprint and causes older ERI artifacts to be reported as
`invalid`, even when their inputs and storage representation are otherwise
unchanged.

## Portable `.rustiq` V1

A portable artifact is a ZIP archive with ZIP64 support, containing a non-empty
collection of independently reusable calculations. Single-calculation archives
use the same V1 format with exactly one entry. The normative root schema is
[`portable-manifest-v1.schema.json`](../schemas/portable-manifest-v1.schema.json).
A [golden H2 manifest](../crates/rustiq-core/tests/data/persistence/portable-manifest-h2-v1.json)
records the ordinary single-calculation case.
The directory cache retains its existing single-artifact manifest and layout.

```text
manifest.json
sources/0                                  optional shared opaque provenance
calculations/calculation-0/request.json     normalized requested state
calculations/calculation-0/calculation.json resolved scientific state
calculations/calculation-0/arrays/integrals/ao-eri.npy
calculations/calculation-1/request.json
calculations/calculation-1/calculation.json
...
```

The root contains `format: "rustiq-persistence"`, `format_version: 1`,
`kind: "portable"`, `producer`, optional `sources`, and `calculations`.
Each calculation entry contains `id`, `scientific_identity`, `request`,
`calculation`, and `artifacts`. There is no root scientific identity or artifact
map. Compatibility remains specific to each calculation and artifact.

Writers assign deterministic identifiers `calculation-0`, `calculation-1`, ...
in collection order. Readers accept unique `calculation-<digits>` identifiers.
Identifiers are references within a snapshot, not scientific identities; neither
identifiers, source order nor user labels enter scientific compatibility.
Request/resolved references and artifacts must stay inside their entry's
`calculations/<id>/` namespace. References are full archive member paths:

```json
"request": {
  "path": "calculations/calculation-0/request.json",
  "version": 1,
  "size": 1234,
  "digest": "sha256:..."
},
"calculation": {
  "path": "calculations/calculation-0/calculation.json",
  "version": 1,
  "size": 1234,
  "digest": "sha256:..."
}
```

`size` and `digest` describe exact uncompressed bytes, including whitespace.
Each snapshot has an independently versioned schema. Producer version is
provenance, not an invalidation rule. An entry may contain no numerical artifacts.
A bundle represents the supplied calculation collection, not an append-only history.
Shared sources are stored once; numerical payload deduplication is not implemented.
The pre-release single-calculation portable layout is replaced by this V1 contract;
there is no migration reader for that unpublished layout.

### Normalized request and optional source provenance

The normative request schema is
[`request-snapshot-v1.schema.json`](../schemas/request-snapshot-v1.schema.json),
with an [H2 golden request](../crates/rustiq-core/tests/data/persistence/request-h2-v1.json).
`request.json` is constructed from `PreparedCalculation::request()`, not source
TOML/XYZ or the resolved snapshot. Its `format: "rustiq-request"` and `version: 1`
identify the independent wire contract. Reopening reconstructs a typed
`CalculationRequest`, without depending on original input files.

The request preserves ordered atoms in the requested `bohr`/`angstrom` units,
charge, multiplicity, a portable basis label, requested `auto`/`rhf`/`uhf` method,
defaulted HF settings, and optional MP2 settings. The requested history size
`diis_size` is required and remains present even when DIIS is disabled.
`diis` and `diis_size` are independent requested values. The requested MP2 memory limit is tagged
`{"kind":"auto"}` or `{"kind":"fixed","bytes":...}`; no machine-resolved
memory budget is stored. A random seed may remain unspecified in the request;
this deliberately asks preparation to choose a fresh seed. The unresolved request
keeps that state while `calculation.json` records the concrete seed actually used.
Comments, source
spans, source paths, cache configuration, and output/UI settings are excluded.
The V1 wire fields remain independent of Rust struct organization: integral screening
is encoded as `hf.eri_schwarz_threshold`, orthogonalization as
`hf.linear_dependency_threshold`, and DIIS as `hf.diis` / `hf.diis_size`.
The decoder maps these to `IntegralConfig`, `OrthogonalizationConfig`, and
`DiisConfig`, respectively.
The basis label is informational; resolved AO contents define scientific identity.

Sources are optional opaque bytes, including non-UTF-8 content. They are never
parsed to reconstruct either semantic snapshot and never enter compatibility.
The optional manifest `sources` array contains records such as:

```json
{"original_name":"../input.toml","path":"sources/0","version":1,
 "size":123,"digest":"sha256:..."}
```

`original_name` is informational provenance and may contain an original path.
It is never used as an extraction or publication path. Writers assign safe member
names (`sources/0`, `sources/1`, ...); readers accept safe relative members under
`sources/` and reject duplicate references. Sources have size/SHA-256 checks and
are read eagerly with a combined 64 MiB limit, at most 256 sources, and at most
4096 UTF-8 bytes per original name. Missing sources are valid for direct Rust API
use. `request.json` and `calculation.json` are required independently of sources.

The reader validates request semantics and their consistency with the resolved
molecule, method, scientific settings, and MP2 selection. Requested Angstrom
coordinates are checked using the V1 constant `0.529177210903` Angstrom per
Bohr, rounded to binary64, and binary64 division. A resolved coordinate may differ
from that result by at most four adjacent binary64 values (ULPs), allowing final
rounding differences between independent implementations. This includes at most
four subnormal steps near zero; there is no absolute chemistry-level tolerance.
Both values and the converted result must be finite. Bohr requests compare
numerically exactly, treating signed zeros as equal. This V1 rule is independent
of domain conversion constants or dependency updates. `calculation.json` remains
authoritative for resolved coordinates and scientific identity; validation never
replaces or rounds them. HF method resolution is also frozen by V1 rather than
delegated to the current RustiQ domain policy: `auto` resolves to RHF for a
closed-shell singlet (multiplicity 1 with an even electron count) and to UHF
otherwise; explicit RHF is valid only for a closed-shell singlet, while explicit
UHF remains UHF. A missing requested random seed may resolve to a generated seed.
Neither snapshot substitutes for the other.

### Resolved scientific snapshot

The normative structural schema is
[`calculation-snapshot-v1.schema.json`](../schemas/calculation-snapshot-v1.schema.json).
The [H2 golden snapshot](../crates/rustiq-core/tests/data/persistence/calculation-h2-v1.json)
provides a complete example. Wire records are explicitly defined, independently
of domain Rust struct layouts and package versions. Iteration limits, DIIS sizes,
and frozen-orbital counts use unsigned 64-bit JSON integers (maximum
18446744073709551615). Domain conversions are checked in both directions;
a reader rejects values exceeding its host integer range.

- `format: "rustiq-calculation"`, `version: 1`, `units: "bohr"` identify the
  snapshot contract and coordinate unit.
- `atoms` contains ordered atomic numbers and positions; `charge` and
  `multiplicity` define the molecular electronic state.
- `basis` contains effective AOs in integral-engine order. Each has a center,
  ordered Cartesian components with three angular-momentum integers, and ordered
  primitives with positive exponents and effective normalized coefficients.
  These include any spherical-component weights already applied by the engine.
  Consumers must not normalize them again. A basis name or source file is not
  needed to interpret the stored AO representation.
- `hf` contains the resolved `rhf`/`uhf` method, iteration limit, convergence and
  linear-dependency thresholds, Schwarz threshold, DIIS history size, and density
  guess. `null`/omitted Schwarz threshold means screening disabled;
  `null`/omitted DIIS size means DIIS disabled.
- Density guesses are tagged as `core_hamiltonian`, `one_electron`, `random`, or
  `zero`. Perturbations and random guesses record distribution parameters and a
  required concrete resolved seed. A missing or null seed is invalid in
  `calculation.json`. Seeds chosen during preparation are captured here, while an
  initially unspecified seed stays unspecified in `request.json`. This does not capture
  generator state or promise exact SCF continuation.
- `mp2` is absent/null for HF-only requests, or contains `frozen_orbitals`.
  This records the request, not an MP2 result or proof of convergence.
- `ao_eri_computation_version` states the scientific algorithm identity used for
  AO ERIs; the snapshot independently reproduces `scientific-identity-v1`.

Coordinates and all floating parameters must be finite. Exponents and positive
thresholds must be greater than zero. Electron counts, multiplicity and RHF method
must be mutually consistent. Random uniform bounds must be finite, ordered, and
have a finite width; normal standard deviation must be positive. Arrays must be
nonempty where the schema requires them. Cross-field scientific checks supplement
the structural JSON schema. Unsupported snapshot versions and unknown fields in
known snapshot records are errors.

The snapshot excludes cache locations, source paths/spans, output presentation,
and memory budgets. Floating-point JSON parsing preserves binary64 round trips,
so reparsing a snapshot cannot change its canonical scientific identity.
The snapshot does not yet provide direct archive execution or reconstruct an SCF
restart state.

### Rust API

`RustiQBundle::new(Vec<RustiQData>)` validates a non-empty portable collection.
`calculations()` and `calculations_mut()` expose entries in manifest order, with
independent typed artifact access and compatibility. The mutable slice cannot
change cardinality. `add_source` / `sources` manage shared provenance; construction
also lifts and deduplicates identical source name/byte pairs attached to entries.
`RustiQBundle::open` reads all request/resolved snapshots but leaves arrays lazy.
`write` publishes all supplied entries as one complete snapshot.

```rust,ignore
use rustiq_core::persistence::{RustiQBundle, RustiQData};

let entries = prepared_calculations.iter()
    .map(RustiQData::from_calculation)
    .collect::<Result<Vec<_>, _>>()?;
let mut bundle = RustiQBundle::new(entries)?;
bundle.add_source("batch.ncl", original_source_bytes)?;
bundle.write("batch.rustiq")?;
let mut reopened = RustiQBundle::open("batch.rustiq")?;
for entry in reopened.calculations_mut() {
    println!("{:?}", entry.calculation().expect("portable context").hf_method());
}
```

The existing single-calculation `RustiQData::open/write` methods delegate to the
same bundle implementation. `open` rejects multi-calculation archives explicitly,
so callers cannot silently drop entries. Sources returned by `RustiQData::open`
are the single bundle's shared provenance. Bundle entries use `bundle.sources()`.


`RustiQData::from_calculation(&PreparedCalculation)` captures requested and resolved inputs
without executing HF. `set_eri` / `set::<AoEriArtifact>` supply an owned
`CompactEri` whose dimension matches those inputs. The caller is responsible for
supplying the ERIs computed for that calculation, rather than unrelated values
of the same shape.

```rust,ignore
use rustiq_core::persistence::{CompactEri, RustiQData};

// `prepared` is produced by CalculationBuilder; `eri` belongs to these inputs.
let mut data = RustiQData::from_calculation(&prepared)?;
// Optional: preserve exact source bytes separately from scientific state.
data.add_source("calculation.toml", original_toml_bytes)?;
data.set_eri(eri)?;
data.write("water.rustiq")?;

let mut restored = RustiQData::open("water.rustiq")?;
let context = restored.calculation().expect("portable context");
println!("{:?}: {} AOs", context.hf_method(), context.basis().len());
for (name, representation) in restored.artifact_representations() {
    println!("{name}: {representation}");
}
if restored.eri_is_compatible(&prepared) {
    let eri: &CompactEri = restored.read_eri()?;
}
```

`request()` exposes the reconstructed typed normalized request. `sources()`
exposes exact captured bytes and informational original names. `calculation()`
exposes read-only typed resolved context, including atoms, effective basis,
HF settings and MP2 request. `producer()` exposes provenance, and
`artifact_representations()` lists known and unknown artifacts without loading
arrays. These inspections do not certify payload integrity.

`open` requires only read access to the source. It validates the container index,
manifest, both snapshot versions/digests/schemas, source sizes/digests, scientific
identity, and declared artifact sizes. ERI dimensions and computation-version
metadata must agree with the snapshot. It does not decode numerical arrays.
The first typed read verifies SHA-256, bounded NPY header, f64 dtype, shape and
exact payload length before decoding. Further reads return the same object.
Missing ERIs are represented by `get::<AoEriArtifact>() == Ok(None)`; an
unsupported representation is never silently treated as usable scientific data.

`eri_is_compatible` compares effective integral inputs with another prepared
calculation without loading arrays. Changing an HF-only request to MP2 does not
invalidate AO ERIs. Compatibility does not imply that the payload has already
been verified. There is no CLI reuse orchestration in this API release.

Every new archive written by RustiQ records producer name `RustiQ` and the
current package version, including when rewriting an older or foreign archive.
Producer provenance does not change scientific identity.

Writing a reopened archive preserves both semantic snapshots and exact captured
source bytes. It preserves unloaded artifacts by verified streaming
copy, including unknown representations and their attributes. Known ERI headers
are checked before preservation; overwritten ERIs use the current representation.
Errors use `PortableError` (including `AlreadyExists`, unsupported version,
invalid context/archive, I/O and artifact errors), without public ZIP types.

### Publication and container policy

Writing creates a sibling temporary file, writes snapshot and artifacts with
SHA-256 metadata, writes the manifest, finalizes the ZIP, syncs the file, and
reopens it to validate the final index/context. Publication uses an atomic
no-clobber operation. **An existing destination is never replaced**, including
when two producers race to create it. Unix additionally syncs the parent
directory after publication. Failures before publication leave the destination
untouched and clean up the temporary file. A directory-sync error after
publication can leave a complete output file and is still reported as an error.

Writers use `Stored` for numerical/opaque artifacts and `Deflated` for
JSON members (`manifest.json`, `request.json`, and `calculation.json`). Streaming
payloads reserve ZIP64 local-header fields even for small arrays, allowing growth
past 4 GiB without buffering. ZIP64 central/end
records are emitted when required. Timestamps are fixed to 1980-01-01 00:00:00,
permissions to regular files with mode 0644, and member order is deterministic.
Repeated writes of the same data by the same implementation produce identical
bytes; compression-library changes are not promised byte-identical output and
never change scientific identity.

### Defensive reading limits

Portable V1 accepts only single-disk ZIP archives with regular file members;
directory entries are unnecessary and rejected. Supported compression is Stored
or Deflated. Encryption, links, devices and other special entry types are errors.
The reader validates original central-directory records before the ZIP library
indexes them, so duplicate names cannot be silently collapsed.

Limits applied before parsing or allocating payloads:

| Structure | Limit |
| --- | ---: |
| Uncompressed manifest | 1 MiB |
| Uncompressed request snapshot, per calculation | 64 MiB |
| Uncompressed calculation snapshot, per calculation | 64 MiB |
| Combined shared sources | 64 MiB |
| Members | 4,096 |
| Central directory | 16 MiB |
| ZIP64 end-record body | 64 KiB |
| NPY header body | 64 KiB |

All member paths obey the logical path rules above. File/ancestor conflicts and
case-insensitive collisions are rejected; metadata paths are reserved. Names must
be valid UTF-8; non-ASCII names require the ZIP UTF-8 flag. Alternative Unicode
path extra fields and duplicate extra fields are rejected to avoid competing
names. Local/central names, flags and compression must agree. Invalid offsets,
overlapping payloads, truncated directory records, and inconsistent ZIP64 fields
are rejected. ZIP metadata names are not extracted onto the filesystem.

Actual decompressed length and ZIP CRC are checked, in addition to independent
SHA-256 digests. Streamed members cannot exceed their declared uncompressed size.
NPY object/pickle arrays, unsupported dtype/rank, dimension overflow, extra bytes,
and incomplete arrays are rejected. Large legitimate ERIs still require memory
for the decoded `CompactEri`; there is no artificial 4 GiB payload limit.

### Interoperability fixtures

`tools/reference/generate_portable_fixtures.py` regenerates Python-produced
ZIP64 fixtures using only standard-library modules and the existing NumPy byte
fixtures. Single- and multi-calculation bundles, shared sources, and both little- and
big-endian f64 payloads are covered. The Python tests
independently reproduce the scientific identity and verify ZIP/JSON/NumPy output
written by Rust. Rust tests also exercise a sparse archive with offsets beyond
4 GiB without allocating a multi-gigabyte payload.

Run `uv run --locked pytest tools/reference` for interoperability and scientific
reference comparisons, separately from the Cargo suite.

## Single-calculation execution APIs

Portable V1 can contain multiple calculations. The current convenience execution
API operates on one calculation, so `RustiQData` represents exactly one entry and
`RustiQData::open` rejects multi-calculation archives. Newly written single-entry
archives use `calculation-0` within the normal V1 bundle layout. Callers should
neither depend on nor manipulate the numeric index.

```rust
// Calculation -> .rustiq
let prepared = CalculationBuilder::new(&geometry, &basis_file).prepare()?;
let hf = prepared.run_hf()?;
let mut data = RustiQData::from_calculation(&prepared)?;
data.write_with_eri("water.rustiq", hf.ao_eri())?;
```

The writer borrows the retained AO ERI without cloning or recomputing it.
`set_eri` remains available when the caller owns the tensor.

```rust
// .rustiq -> Calculation
let data = RustiQData::open("water.rustiq")?;
let prepared = data.prepare_calculation()?;
let result = prepared.execute()?;
let eri = result.hf.ao_eri();
```

`prepare_calculation` consumes the data facade and restores the resolved molecule,
AO basis, and scientific options. It retains the archive as a read-only artifact
source. Payloads are decoded lazily when execution needs them. Compatible AO ERIs
are verified and reused automatically by `execute`, `execute_with_events`, and
`run_hf`; missing or incompatible artifacts fall back to the enabled local cache
and then computation. Corrupt or unsupported compatible payloads fail explicitly.
The source archive is never modified.

A new calculation can also reuse a previous archive with different requested options:

```rust
let prepared = CalculationBuilder::new(&geometry, &basis_file)
    .with_mp2(Mp2Config::default())
    .prepare()?
    .with_reuse_data(RustiQData::open("water-hf.rustiq")?)?;
let result = prepared.execute()?;
```

The execution interface is independent of the input source. An internal enum
distinguishes configuration-only execution from portable artifact reuse. The
resolver uses the existing sealed `Artifact` markers for presence, compatibility,
and loading, rather than archive paths or a public storage/backend trait. Future
HF solution and restart markers can participate at their respective stages through
the same resolver; those representations are not implemented yet.

`CalculationEvent::ArtifactReuse` reports per-artifact reused, missing, or
incompatible decisions. Its decision type also distinguishes intentionally ignored
state for future selection policies. Computed and reused values are retained in a
separate in-memory typed data facade for subsequent executions. Compact ERI clones
share immutable storage; mutation detaches that storage. Neither ordinary reuse nor
this memory retention rewrites the source snapshot or stores runtime scratch data.

For callers managing tensors explicitly, `take_compatible_eri`,
`execute_with_eri`, and `run_hf_with_eri` remain available. This explicit path
rejects missing, incompatible, corrupt, or unsupported artifacts, and validates
tensor dimensions. ERIs remain accessible in the HF solution and are reused by MP2.

A complete H2/STO-3G example set is in
[`crates/rustiq-core/examples`](../crates/rustiq-core/examples). Each file shows
one workflow. `calculate_from_rustiq` uses only the archive; the other examples
load geometry and basis from repository sample and test data:

```sh
cargo run -p rustiq-core --example hf_calculation
cargo run -p rustiq-core --example write_rustiq
cargo run -p rustiq-core --example read_rustiq
cargo run -p rustiq-core --example calculate_from_rustiq
cargo run -p rustiq-core --example copy_rustiq
```

The examples read and write archives in the ignored
`crates/rustiq-core/examples/data/` directory. The read examples expect
`h2.rustiq` to have been created by `write_rustiq`.


## CLI publication and bundle selection

`run --artifact PATH` writes the current non-empty calculation collection using
this V1 format. TOML and single Nickel inputs produce one entry; Nickel batches
produce one entry per requested calculation. Exact main-input and geometry bytes
are optional shared provenance, not scientific compatibility inputs. Imports are
not recursively captured.

`run --reuse PATH` opens the bundle read-only. `RustiQBundle::select_ao_eri`
selects the first compatible, supported AO ERI in manifest order for each effective
calculation, independently of labels or source order. Missing or incompatible
state falls through to an enabled local cache and then computation. A corrupt
selected payload is an explicit error. Selection can also return `Ignored` for
an internally disabled reuse policy, without decoding or deleting the artifact.
The CLI does not expose selection filters.

`RustiQBundle::write` publishes without overwriting. `RustiQBundle::replace`
writes, finalizes, syncs and validates a complete sibling temporary archive before
atomic replacement. The CLI uses replacement only when `--reuse` and `--artifact`
resolve to the same canonical path; symbolic aliases preserve the symlink and
replace its target. Other existing destinations, including different hard links,
are rejected. Source archive handles are closed before replacement. Failures
before publication leave the previous bytes intact.

The CLI publishes only after all requested calculations have produced results.
Non-converged HF calculations can contribute their AO ERI; HF state itself is not
persisted. Any preparation or execution error prevents publication of the whole
collection. New snapshots contain only the current requested collection and its
compatible known artifacts, not a history of the source bundle.

`artifact inspect` uses typed versioned snapshots without arbitrary extraction or
scientific execution. It lists unknown artifact representations too. Inspection
validates archive structure and request/resolved snapshots, but does not claim
full numeric payload verification. Requested cache and terminal settings are
not reconstructed because they are frontend policy, absent from the scientific
snapshots. See the README for commands and JSON presentation contracts.
