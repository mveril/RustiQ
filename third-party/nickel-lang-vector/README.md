# Nickel vector dependency backport

The files in `src/` are unchanged from the published `nickel-lang-vector`
0.2.0 crate. RustiQ keeps Nickel 0.19.0 and its vector API while changing
`imbl-sized-chunks` from 0.1 to 0.2 in this crate's standalone manifest.
The fixed dependency addresses
[RUSTSEC-2026-0292](https://rustsec.org/advisories/RUSTSEC-2026-0292).
Unused upstream benchmark/dev dependencies are omitted from this backport.

The source archive supplies this directory and its manifest through the root
`[patch.crates-io]` entry. Remove the backport when a compatible upstream
Nickel vector release uses the fixed dependency. The code retains upstream MIT
licensing; `LICENSE` reproduces the upstream repository's MIT copyright notice
whose provenance is recorded in `licenses/extra-notices.txt`.

Check the upstream unit tests with:

```sh
cargo test --manifest-path third-party/nickel-lang-vector/Cargo.toml --lib
```
