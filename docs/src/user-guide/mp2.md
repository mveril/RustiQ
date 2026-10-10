# MP2 calculations

Second-order Møller–Plesset perturbation theory (MP2) estimates an electron
correlation correction using molecular orbitals from a converged HF reference.
It does not replace HF: RustiQ first solves HF, then evaluates MP2. MP2 is
available with both RHF and UHF references. An unconverged HF result cannot be
used as an MP2 reference.

The checked-in H₂ input requests RHF-MP2 with no frozen orbitals:

```toml
{{#include ../../../samples/h2/sto-3g/mp2_calculation.toml}}
```

Run it from the repository root after importing STO-3G:

```sh
cargo run --locked -- basis import tests/data/sto-3g.json
cargo run --locked -- run samples/h2/sto-3g/mp2_calculation.toml
```

The MP2 correlation energy is the correction added to HF electronic energy.
Total MP2 energy is HF electronic energy plus MP2 correlation energy plus
nuclear repulsion. The correlation energy is usually negative, but its sign
and size depend on the system and reference.

## Open-shell UHF-MP2 example

OH is an open-shell doublet (multiplicity 2). For this input, `Auto` selects
UHF, and MP2 uses the converged UHF reference. The existing OH/STO-3G input is:

```toml
{{#include ../../../samples/oh/sto-3g/mp2_calculation.toml}}
```

Run it from the repository root after making STO-3G available, as described in
the [basis-set guide](basis-sets.md):

```sh
cargo run --locked -- run samples/oh/sto-3g/mp2_calculation.toml
```

The calculation reports a UHF-MP2 correlation energy. Existing PySCF
comparisons cover this specific example within the tolerances documented
below; they do not establish general chemical accuracy.

## Frozen occupied orbitals

Occupied orbitals contain electrons in the HF reference; virtual orbitals are
unoccupied orbitals available as excitation destinations. The
`frozen_orbitals` setting excludes the lowest occupied orbitals from the MP2
correlation treatment. Its default is zero, so all occupied orbitals are
included. RHF requires at least one occupied orbital to remain correlated and
at least one virtual orbital; the frozen count must be smaller than the
occupied count. For UHF, the count
cannot exceed either spin's occupied-orbital count, though one spin sector may
be fully frozen.

For RHF, the count refers to spatial orbitals; freezing one excludes one
doubly occupied orbital. For UHF, the same count is applied separately to the
alpha and beta occupied spaces. The excluded orbitals can have different
spatial shapes. The setting does not choose chemically defined core orbitals
automatically. Select it explicitly when comparing calculations.

## Memory use

MP2 transforms integrals from the atomic-orbital basis into molecular-orbital
blocks. RustiQ processes these blocks to avoid storing the full transformed
four-index tensor at once. `memory_limit` configures the estimated additional
workspace used for these transformations. `"auto"` selects a budget based on
available memory; explicit byte sizes such as `"500 MB"` can also be set.

This workspace budget is **not a limit on total process memory**. It excludes
already resident HF data and atomic-orbital integrals, allocator overhead, and
other numerical-library scratch space. Even a single required block may exceed
a very small budget. See the source for
[`Mp2MemoryPlan`](https://github.com/mveril/RustiQ/blob/main/crates/rustiq-core/src/mp2/blocked.rs)
and the authoritative Nickel defaults in the
[input contract](../reference/index.md).

For example, the existing Nickel default is `auto`. An explicit byte-size
budget belongs under `[method.mp2]`:

```toml
[method.mp2]
memory_limit = "500 MB"
```

If the budget is below the minimum workspace needed for one occupied block,
that sector cannot be transformed and the MP2 calculation errors. Automatic
budget resolution follows the shared [resource policy](https://github.com/mveril/RustiQ/blob/main/crates/rustiq-core/src/resources.rs).

The H₂/STO-3G RHF-MP2 comparison checks the HF energy within \\(2\times10^{-10}\\)
Hartree and MP2 correlation energy within \\(10^{-10}\\) Hartree against PySCF
2.14.0. The OH/STO-3G UHF-MP2 case checks HF energy within \\(5\times10^{-8}\\),
correlation energy within \\(5\times10^{-9}\\) Hartree, and spin expectation
within \\(2\times10^{-5}\\). These are agreement tolerances for those inputs,
not a measure of general chemical accuracy. See the
[reference coverage](../scientific-scope.md#reference-comparisons) and the
[reference test source](https://github.com/mveril/RustiQ/blob/main/tools/reference/test_compare_pyscf.py).
Reproduce both selected cases with:

```sh
nix run .#pyscf-check -- -k 'h2-sto-3g-rhf-mp2 or oh-sto-3g-uhf-mp2'
```

Small energy denominators can make MP2 corrections unreliable, especially in
strongly correlated or near-degenerate systems. Numerical safeguards and
agreement on a few examples do not establish physical validity. The method's
perturbative basis is described in
[Møller and Plesset (1934)](https://journals.aps.org/pr/abstract/10.1103/PhysRev.46.618);
see also this discussion of problematic MP2 denominators
([Lee and Head-Gordon, 2018](https://arxiv.org/abs/1807.06185)).
