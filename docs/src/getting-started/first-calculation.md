# First H₂ calculation

## Using only the release binary

Copy the following geometry into a file named `h2.xyz` in your working
directory. It is the same geometry used by the checked-in sample:

```text
{{#include ../../../samples/h2/molecule.xyz}}
```

The explicit download is needed because `run` does not fetch missing bases
unless `--auto-download` or `RUSTIQ_AUTO_DOWNLOAD` enables downloading.
Use `--no-auto-download` to explicitly prevent it. The release archive contains neither
sample inputs nor basis files. To use an existing local basis file without
network access, import it with `rustiq basis import /path/to/basis.json`.


Create an input, download STO-3G, and run the calculation:

```sh
rustiq init h2.xyz -o calculation.toml
rustiq basis download sto-3g
rustiq run calculation.toml
```

> [!TIP]
> With an online-enabled build and network access, you can replace the separate download and run commands
> with `rustiq run calculation.toml --auto-download` to fetch a missing basis.

## Reproducing the sample configuration without a checkout

The checked-in configuration is included directly:

```toml
{{#include ../../../samples/h2/sto-3g/calculation.toml}}
```

You can also copy this configuration without a checkout: save the geometry
shown above as `molecule.xyz` in a new working directory, create an `inputs`
directory with `mkdir inputs`, and save the TOML as `inputs/h2.toml`. After
obtaining STO-3G as described above, run `rustiq run inputs/h2.toml`. The
`../molecule.xyz` path resolves relative to the runfile directory. This is an
alternative to the `init` workflow; keep using `calculation.toml` from that
workflow in subsequent guides.

## Using the checked-in sample

From a source checkout, run these commands from the repository root to import
the checked-in fixture and run the example without installing RustiQ:

```sh
cargo run --locked -- basis import tests/data/sto-3g.json
cargo run --locked -- run samples/h2/sto-3g/calculation.toml
```

CLI tests import the same fixture into an isolated store.

## Verifying the calculation

Default input units are Angstrom; resolved coordinates are Bohr. The neutral
singlet defaults resolve HF `Auto` to RHF. The core-Hamiltonian guess makes
this example deterministic.

The following verification commands are for users working from a source
checkout; they are optional when using only the release binary. The existing CLI
regression test checks this input, RHF resolution, units, two
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

Reproducing this comparison requires the cloned repository and its Nix
development environment (or the documented `uv`/PySCF setup); the binary
archive alone does not include the reference harness.

```sh
nix run .#pyscf-check -- -k 'h2-sto-3g-rhf and not mp2'
```

These checks validate this case, not arbitrary chemistry.
**Results remain experimental and require independent validation before research use.**
Continue with [output interpretation](output.md).
To save and reuse the calculation's AO ERIs, continue with the
[portable bundle guide](../user-guide/portable-bundles.md).

Explore more calculations and workflows in the [examples index](../user-guide/examples.md).
