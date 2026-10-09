# Contributing and the Rust API

[CONTRIBUTING.md](https://github.com/mveril/RustiQ/blob/main/CONTRIBUTING.md)
owns branch naming, lint policy, and verification commands.
See [development environments](environments.md) for setup.

The workspace keeps CLI parsing and presentation in the root package and
scientific configuration and calculations in `rustiq-core`. The core's
`calculation` module orchestrates work in `molecules`, `basis`, `eri`,
`hf`, and `mp2`.

Rustdoc remains the canonical public API reference. Generate and open it with
MathJax support:

```sh
tools/generate-rustdoc.sh --open
```

On Windows:

```powershell
.\tools\generate-rustdoc.ps1 -Open
```

The generated entry point is `target/doc/rustiq_core/index.html`.
MathJax equations require browser access to the CDN.
The initial Pages site publishes the manual; rustdoc is generated separately.

[Runnable API examples](https://github.com/mveril/RustiQ/tree/main/crates/rustiq-core/examples)
and [public API tests](https://github.com/mveril/RustiQ/blob/main/crates/rustiq-core/tests/public_api.rs)
provide executable workflows and verification. Link to these sources instead
of duplicating item-level documentation in the book.
