# First H₂ calculation

Run neutral singlet H₂ with STO-3G from the repository root:

```sh
cargo run --locked -- basis import tests/data/sto-3g.json
cargo run --locked -- run samples/h2/sto-3g/calculation.toml
```

Import writes the checked-in basis fixture to the application's local basis
store, avoiding a network download. CLI tests import the same fixture into
an isolated store.

The checked-in configuration is included directly:

```toml
{{#include ../../../samples/h2/sto-3g/calculation.toml}}
```

The geometry path resolves from the runfile directory. Preserve the directory
layout if copying the sample. Its XYZ source is:

```text
{{#include ../../../samples/h2/molecule.xyz}}
```

Default input units are Angstrom; resolved coordinates are Bohr. The neutral
singlet defaults resolve HF `Auto` to RHF. The core-Hamiltonian guess makes
this example deterministic.

The existing CLI regression test checks this input, RHF resolution, units, two
SCF iterations, and these selected output lines. Its reproducible command is
below; the test source is linked for readers who want to inspect its assertions.

[CLI test source on GitHub](https://github.com/mveril/RustiQ/blob/main/tests/cli_samples.rs)

```text
SCF converged after 2 iterations.
Total Energy (including nuclear repulsion): -1.116759 Hartree
```

Run that verification with:

```sh
cargo test --locked --test cli_samples test_cli_h2_sample_converges_and_prints_reference_energy
```

For independent numerical verification, the `h2-sto-3g-rhf` case compares full
precision JSON energies against PySCF with a `2e-10` Hartree energy tolerance.
The rounded terminal value above is not that tolerance. See the book's
[scientific scope and reference comparisons](../scientific-scope.md#reference-comparisons)
for the coverage boundary. The comparison implementation and full reference
instructions are linked below.

[Comparison source on GitHub](https://github.com/mveril/RustiQ/blob/main/tools/reference/compare_pyscf.py)

[Reference test instructions on GitHub](https://github.com/mveril/RustiQ/blob/main/tools/reference/README.md)

```sh
nix run .#pyscf-check -- -k h2-sto-3g-rhf
```

These checks validate this case, not arbitrary chemistry.
**Results remain experimental and require independent validation before research use.**
Continue with [output interpretation](output.md).
