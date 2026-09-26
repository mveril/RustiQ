# RustiQ logical persistence format V1

This document specifies the storage-independent logical format used by RustiQ
scientific persistence. It does not specify an archive container.

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

The core Rust API exposes `persistence::RustiQData` for reading and writing a
directory in this format. `RustiQData::read` reads the manifest only. Its
`read_eri` method validates and decodes the AO ERI NPY on first access and keeps
the resulting `CompactEri` for subsequent accesses. Writing to a new directory
copies artifacts that have not been decoded, including unknown representations,
as verified byte streams. Its public scientific API is typed: `set_eri` and
`read_eri` operate on `CompactEri`. The generic `get::<AoEriArtifact>()` returns
`Result<Option<&CompactEri>, PersistenceError>`, while
`set::<AoEriArtifact>(value)` accepts only `CompactEri`; only declared artifact
marker types are accepted. A future known artifact gets its own marker, typed field,
and accessors in `RustiQData`. The `NpyConvert` trait handles NPY byte streams
for `CompactEri` and `DMatrix<f64>`. Its associated `Shape` type is `usize` for
the ERI basis-function count and `(usize, usize)` for matrix dimensions;
`try_read_with_shape` checks the declared shape before constructing the value.
`from_npy` and `try_from_npy_with_shape` also accept an already parsed
`npyz::NpyFile`, so callers can parse a stream once without using a filesystem
path. Matrix readers accept C and Fortran order and matrix writers emit Fortran
order.

- `manifest.json` is UTF-8 JSON and describes the format, producer, scientific
  identity and artifacts.
- `arrays/integrals/ao-eri.npy` is the AO electron-repulsion integral artifact.

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

The root manifest is an index of versioned scientific artifacts rather than a
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
