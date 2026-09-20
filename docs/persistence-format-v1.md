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
basis-function count, payload size and SHA-256 digest before reading NPY data.
Any missing, malformed, stale, truncated or corrupted entry is a cache miss and
must be recomputed; it is never used as scientific input. Entries are written
to a sibling temporary directory, finalized and synced, then atomically renamed
into place. Published entries are immutable, so concurrent producers can safely
leave the first completed entry in place.

Use `rustiq cache list` to inspect published entries. To delete precisely one
entry, pass its full fingerprint to `rustiq cache remove <fingerprint>`; the
command accepts only a 64-character lowercase SHA-256 digest and deletes only
the matching entry directory. `rustiq cache remove --all` removes all published
AO ERI entries under the selected cache root.

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
