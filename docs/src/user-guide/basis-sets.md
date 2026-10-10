# Gaussian basis sets

A basis set is a finite collection of functions used to represent molecular
orbitals. RustiQ's Gaussian basis functions are built from Gaussian functions
centered on atoms. The chosen basis affects the size of the orbital space and
the numerical result. A small basis such as STO-3G is useful for examples but
does not guarantee accurate energies or properties.

## Offline workflow

Import the checked-in STO-3G basis fixture into RustiQ's local basis store and
run the H₂ example:

```sh
cargo run --locked -- basis import tests/data/sto-3g.json
cargo run --locked -- run samples/h2/sto-3g/calculation.toml
```

The import is local and does not need network access. The test suite exercises
the same fixture using an isolated store in
[`tests/cli_samples.rs`](https://github.com/mveril/RustiQ/blob/main/tests/cli_samples.rs)
and basis import/storage behavior in
[`crates/rustiq-core/src/basis/basis_store.rs`](https://github.com/mveril/RustiQ/blob/main/crates/rustiq-core/src/basis/basis_store.rs).

List, import, or remove locally stored bases with:

```sh
cargo run --locked -- basis list
cargo run --locked -- basis import tests/data/sto-3g.json
cargo run --locked -- basis remove sto-3g
```

The default store is in the platform's application data directory. Set
`RUSTIQ_DATA_BASIS` to choose its exact path, or `RUSTIQ_DATA_HOME` to choose
the data root. The CLI guide in
[existing workflows](../reference/existing-workflows.md#basis-management-and-storage)
lists platform paths and migration notes.

## Online workflow

The default CLI build enables the `online` feature. Use it to list or download
bases:

```sh
cargo run --locked -- basis list --online
cargo run --locked -- basis download sto-3g
```

Online commands require network access. A normal `run` uses the local basis
store; automatic downloading can be enabled with `--auto-download`, disabled
with `--no-auto-download`, or set using `RUSTIQ_AUTO_DOWNLOAD`. Builds without
the `online` feature can import, list local files, and remove bases but cannot
download or query online listings. The commands reflect the CLI tests in
[`tests/cli_samples.rs`](https://github.com/mveril/RustiQ/blob/main/tests/cli_samples.rs).

## Supported input features

Basis files are imported in the Basis Set Exchange JSON form. A shell groups
functions with the same center and angular momentum: \\(l=0,1,2\\) denote
s, p, and d functions. Cartesian and spherical describe two function forms
that differ in how angular dependence is represented. For elements used by a
molecule, the current loader accepts Gaussian-type and Cartesian Gaussian
shells through angular momentum \\(l=15\\), and spherical Gaussian shells
through \\(l=2\\). It rejects Slater-type shells and effective core
potentials (ECPs). Higher angular momentum or other basis-file features should
not be assumed to work just because a basis is downloadable. This describes
loader support, not numerical validation of all those cases. The checks are in
[`basis.rs`](https://github.com/mveril/RustiQ/blob/main/crates/rustiq-core/src/basis/gaussian/basis.rs).

Basis choice is one part of a calculation's accuracy. Convergence, independent
agreement, basis quality, and physical validity are distinct checks; see
[scientific scope](../scientific-scope.md).
See the [glossary](../glossary.md) for shared basis vocabulary.

Next: [TOML configuration](configuration.md).
