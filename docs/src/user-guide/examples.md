# Examples

This curated selection introduces introductory and representative workflows; it is
not an exhaustive catalog. See the repository’s
[samples directory](https://github.com/mveril/RustiQ/tree/main/samples)
for additional molecules and advanced configurations. The binary archives omit the samples. If you installed a release binary,
download and extract the **Source code** archive for the same version from
[GitHub Releases](https://github.com/mveril/RustiQ/releases). Run the commands
from the extracted source directory. Use fresh output filenames when repeating
a tutorial, or pass `--force` to replace bundle and Molden outputs.

These guides use the installed `rustiq` executable. Basis sets are not distributed
with RustiQ: download them explicitly, or import independently obtained JSON files
with `rustiq basis import /path/to/basis.json` when offline. Automatic downloads
must be enabled explicitly with `--auto-download` or
`RUSTIQ_AUTO_DOWNLOAD`; `--no-auto-download` prevents them. See
[basis availability](basis-sets.md) for offline and online builds.

| Example and objective | Basis | Guide and prerequisites |
| --- | --- | --- |
| Water / RHF and MP2 | STO-3G | [Reuse a saved RHF reference](#water--run-mp2-from-saved-rhf-orbitals). |
| Benzene / RHF | STO-3G | [Export molecular orbitals](#benzene--export-molecular-orbitals). |
| H₂ / RHF: first calculation | STO-3G | [First calculation](../getting-started/first-calculation.md); create the XYZ file and obtain the basis. |
| OH / UHF: open-shell calculation | STO-3G | [Inspect an unrestricted solution](#hydroxyl-radical--inspect-an-unrestricted-solution). |
| H₂ / RHF-MP2: electron correlation | STO-3G | [MP2](mp2.md); first create H₂ as in the first calculation. |
| OH / UHF-MP2: open-shell correlation | STO-3G | [Open-shell MP2](mp2.md#open-shell-uhf-mp2-example); create OH as in the HF guide. |
| Inspect and transform XYZ geometry | None | [Geometry](geometry.md); create an XYZ file. |
| Parameterized and batch studies | STO-3G and 6-31G | [Nickel studies](nickel-studies.md); obtain both bases. No separate Nickel executable is needed. |
| Create, inspect, verify and rerun bundles; reuse AO ERIs | STO-3G | [Portable bundles](portable-bundles.md); first prepare the H₂ input and basis. |

## Water — run MP2 from saved RHF orbitals

In a source checkout, run these commands from the repository root with an
installed `rustiq`. Alternatively, use `cargo run --locked --` in place of
`rustiq`. Import the STO-3G basis before your first run.

```sh
rustiq basis import tests/data/sto-3g.json
rustiq run samples/h2o/sto-3g/calculation.toml --artifact water-hf.rustiq
rustiq artifact verify water-hf.rustiq
rustiq run water-hf.rustiq --post-scf samples/h2/sto-3g/postscf.toml --artifact water-mp2.rustiq --format json > water-mp2.json
```

The neutral singlet has five doubly occupied spatial orbitals. The Post-SCF
command reads the converged RHF reference and AO integrals from
`water-hf.rustiq`. A plain `rustiq run water-hf.rustiq` starts a fresh SCF
calculation. The MP2 settings file can be shared across molecules because it
contains only method settings.

In `water-mp2.json`, look for one successful calculation and an RHF reference.
For this input, the total MP2 energy is approximately −74.998630 Hartree,
the sum of the total HF energy and a correlation energy of approximately
−0.035567 Hartree. Keep the JSON report for the energies and the bundle for the
reference, scientific inputs, and applied settings. The split-run test
compares fresh and resumed MP2 energies to within 10⁻¹² Hartree. Independent
water MP2 comparisons use 6-31G and cc-pVDZ. STO-3G is a minimal basis for
learning the workflow. See the [reference checks](https://github.com/mveril/RustiQ/tree/develop/tools/reference)
and [MP2 limitations](mp2.md).

From a development checkout, reproduce the split-run check with
`cargo test --locked --test artifact_cli post_scf_split_run_matches_full_mp2_calculation`.
For the independent water MP2 and OH HF benchmarks, run
`uv run --locked pytest tools/reference/test_compare_pyscf.py -k "h2o and mp2 or oh-sto-3g-uhf"`.

## Hydroxyl radical — inspect an unrestricted solution

With STO-3G imported, run the neutral doublet from the repository root.

```sh
rustiq run samples/oh/sto-3g/calculation.toml --artifact oh-hf.rustiq
rustiq artifact export oh-hf.rustiq --format molden -o oh.molden
```

Expect UHF, a total HF energy of about −74.362669 Hartree, and five occupied
alpha and four occupied beta orbitals. Inspect
the `Spin=` and `Occup=` records under `[MO]` in `oh.molden`. Each occupied
spin orbital has occupation 1. The ideal doublet has ⟨S²⟩ = 0.75. The
difference between the reported value and 0.75 measures spin contamination,
which can persist after SCF convergence. The PySCF reference suite compares
this STO-3G HF energy within 5 × 10⁻⁸ Hartree and ⟨S²⟩ within 2 × 10⁻⁵.
See [Hartree–Fock diagnostics](hartree-fock.md).

## Benzene — export molecular orbitals

Run from the repository root. Benzene requires STO-3G data for carbon and
hydrogen. The water fixture covers hydrogen and oxygen. Download the full
basis with `rustiq basis download sto-3g`, or import a Basis Set Exchange JSON
file that includes carbon and hydrogen with `rustiq basis import /path/to/full-sto-3g.json`.
Either operation replaces a stored STO-3G fixture.

```sh
rustiq run samples/benzene/sto-3g/calculation.toml --artifact benzene-hf.rustiq
rustiq artifact export benzene-hf.rustiq --format molden -o benzene.molden
```

Expect 21 doubly occupied RHF spatial orbitals. In the Molden file, these
have occupation 2. Virtual orbitals have occupation 0. The symmetry field
is a placeholder. For file conventions and supported shells, see
[Molden export](portable-bundles.md#export-orbitals-to-molden). The independent
interoperability tests read RustiQ exports with PySCF and reconstruct the
orbitals for smaller systems, including water and hydroxyl.

To see the orbital shape, use the example Python plotter. With NumPy, PySCF,
and Matplotlib installed in a Python environment, run the following command.
Install the dependencies with `python -m pip install numpy pyscf matplotlib`.

```sh
python samples/benzene/plot_molden.py benzene.molden benzene-homo.png
```

The plotter reads the exported file with PySCF and evaluates its highest
occupied molecular orbital in the plane z = 1 Bohr, approximately 1 Bohr
above the ring. Open the PNG in an image
viewer. The two colors show opposite signs of the orbital amplitude, and
the dots mark projected atomic positions. The overall sign is arbitrary.
Degenerate orbitals can rotate within their shared subspace.

![Benzene HOMO slice plotted from the RustiQ Molden export](images/benzene-homo.png)

Report scientific discrepancies through the [issue tracker](https://github.com/mveril/RustiQ/issues).
Include the RustiQ version, input and geometry, basis source, observed and
expected results, comparison method, and units. Use the same tracker for
feature requests and workflow feedback.
