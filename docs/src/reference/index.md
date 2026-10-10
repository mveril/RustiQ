# Authoritative references

These documents remain the sources of truth:

- [Command-line reference](cli.md): shipped commands, principal options,
  output behavior, and availability boundaries.
- [TOML calculation configuration](../user-guide/configuration.md) and
  [Nickel studies](../user-guide/nickel-studies.md): runnable inputs linked to
  the authoritative Nickel defaults and validation below.

- [Persistence V1 specification](https://github.com/mveril/RustiQ/blob/main/docs/persistence-format-v1.md):
  the portable ZIP/ZIP64 `.rustiq` contract.
- [Calculation JSON schema](https://github.com/mveril/RustiQ/blob/main/schemas/calculation-output-v1.schema.json)
  and [batch JSON schema](https://github.com/mveril/RustiQ/blob/main/schemas/batch-output-v1.schema.json).
- [Embedded Nickel schema](https://github.com/mveril/RustiQ/tree/main/src/runfile/nickel):
  input defaults and validation.
- [Rustdoc](../development/contributing.md): public Rust API reference.
- [CONTRIBUTING.md](https://github.com/mveril/RustiQ/blob/main/CONTRIBUTING.md):
  contribution requirements.
- [CITATION.cff](https://github.com/mveril/RustiQ/blob/main/CITATION.cff):
  project citation metadata.
- [ROADMAP.md](https://github.com/mveril/RustiQ/blob/main/ROADMAP.md)
  and [issues](https://github.com/mveril/RustiQ/issues): future work.

The directory-backed local AO ERI cache is disposable, machine-local storage.
A portable `.rustiq` bundle is a deliberate scientific snapshot accessed by
the current Rust APIs. A cache directory is not a portable bundle.
