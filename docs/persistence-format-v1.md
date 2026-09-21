# RustiQ logical persistence format V1

This document specifies the storage-independent logical format used by RustiQ
scientific persistence. It does not specify an archive container.

## Active AO ERI cache

The active deterministic-integral cache is directory-backed. Its root is an
explicit application choice; the scientific core never chooses a user or
system cache directory. The RustiQ CLI defaults to `dirs::cache_dir()/RustiQ`,
or `RUSTIQ_CACHE_HOME` when set; `rustiq run --cache-dir DIR` overrides it
for one execution. An AO ERI entry has this layout:

```text
<cache-root>/eri/<scientific-identity-digest>/
├── manifest.json
└── arrays/integrals/ao-eri.npy
```

The cache validates the manifest version, scientific identity, representation,
basis-function count, payload size, SHA-256 digest and NPY header before reading
NPY data. `rustiq cache list` reports entries as `verified` only after these
bounded-memory checks; it does not load the ERI values.
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

- `manifest.json` is UTF-8 JSON and describes the format, producer, scientific
  identity and artifacts.
- `arrays/integrals/ao-eri.npy` is the AO electron-repulsion integral artifact.

Every artifact records its logical path, byte size, representation,
basis-function count and a content digest in the form `sha256:<lowercase hex>`.
NPY-specific metadata such as the dtype and logical shape is carried by the NPY
header rather than duplicated in the manifest. Readers should validate the
declared size and digest before scientific use. Payload integrity is independent
of its container.

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

1. identity and ERI representation identifiers;
2. ordered atoms: atomic number and coordinates in Bohr;
3. ordered effective AO data consumed by the ERI engine: shell center,
   normalized component angular momentum, primitive exponents and effective
   normalized coefficients;
4. Schwarz screening state, encoded distinctly as disabled or enabled with a
   positive finite threshold.

Producer version, paths, timestamps, compression and container metadata do not
participate in scientific identity.
