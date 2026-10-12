# TOML calculation configuration

Examples in this chapter that use `samples/` or `tests/data/` require a cloned
repository and should be run from its root. Release binaries do not include
those files; use equivalent input files in your working directory instead.

A TOML input describes one calculation. Begin with the checked-in neutral H₂
example:

```toml
{{#include ../../../samples/h2/sto-3g/calculation.toml}}
```

From the repository root, import the basis if needed and run the input:

```sh
rustiq basis import tests/data/sto-3g.json
rustiq run samples/h2/sto-3g/calculation.toml
```

The `[molecule]` section points to an XYZ geometry and can set `charge`,
`multiplicity`, and coordinate `units`. `[basis]` names a locally available
basis. `[method.hf]` controls Hartree–Fock, and `[method.mp2]` requests a
correlation calculation after HF. See the [HF](hartree-fock.md) and
[MP2](mp2.md) guides for interpreting those methods.

For example, the existing MP2 sample adds an MP2 section:

```toml
{{#include ../../../samples/h2/sto-3g/mp2_calculation.toml}}
```

Defaults such as neutral charge, Angstrom coordinates, automatic HF selection,
100 SCF iterations, and a \\(10^{-8}\\) convergence threshold are filled by the
embedded Nickel contracts. The same contracts reject unknown fields and
validate values; TOML parsing alone does not describe the full input rules.
See the [authoritative Nickel input contracts](../reference/index.md).

Important HF numerical controls include `convergence_threshold`,
`max_iterations`, `diis.enabled`, `diis.max_history`, and
`orthogonalization.linear_dependency_threshold`. MP2 has `frozen_orbitals`
and `memory_limit`; the latter is a workspace budget, not a process memory
limit. The defaults and detailed validation rules belong to the Nickel source,
not a second specification in this guide.

## Optional local AO ERI cache

The directory-backed AO ERI cache is local, disposable storage. It is disabled
by default; enable it in the runfile to allow reuse across runs:

```toml
[cache]
enabled = true
```

`--cache-dir DIR` selects the directory for this run but does not enable the
cache. The cache can be used as a fallback when a portable bundle has no
compatible AO ERI; it is not itself a portable `.rustiq` bundle. See the
[portable bundle guide](portable-bundles.md) for that reuse order.

The geometry path is resolved relative to the TOML file. If the input is read
from standard input, its geometry path is resolved from the current working
directory. Geometry file structure and coordinate units are explained in the
[geometry guide](geometry.md).

The CLI reports diagnostics for malformed TOML, unknown or invalid fields,
unavailable bases, and invalid molecular states. Tests cover valid examples
and grouped diagnostics in
[`tests/cli_samples.rs`](https://github.com/mveril/RustiQ/blob/main/tests/cli_samples.rs)
and [`tests/nickel_cli.rs`](https://github.com/mveril/RustiQ/blob/main/tests/nickel_cli.rs).
Successful validation means the requested input is internally acceptable; it
does not validate the scientific choice of state or method.
See the [glossary](../glossary.md) for shared configuration and scientific
terms.
