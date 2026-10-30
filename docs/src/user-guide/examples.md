# Examples

This curated selection introduces introductory and representative workflows; it is
not an exhaustive catalog. See the repository’s
[samples directory](https://github.com/mveril/RustiQ/tree/main/samples)
for additional molecules and advanced configurations. Those files require a source
checkout and are not bundled with release archives.

These guides use the installed `rustiq` executable. Basis sets are not distributed
with RustiQ: download them explicitly, or import independently obtained JSON files
with `rustiq basis import /path/to/basis.json` when offline. Automatic downloads
must be enabled explicitly with `--auto-download` or
`RUSTIQ_AUTO_DOWNLOAD`; `--no-auto-download` prevents them. See
[basis availability](basis-sets.md) for offline and online builds.

| Example and objective | Basis | Guide and prerequisites |
| --- | --- | --- |
| H₂ / RHF: first calculation | STO-3G | [First calculation](../getting-started/first-calculation.md); create the XYZ file and obtain the basis. |
| OH / UHF: open-shell calculation | STO-3G | [Hartree–Fock](hartree-fock.md#select-a-method-and-starting-guess); create the OH geometry and obtain the basis. |
| H₂ / RHF-MP2: electron correlation | STO-3G | [MP2](mp2.md); first create H₂ as in the first calculation. |
| OH / UHF-MP2: open-shell correlation | STO-3G | [Open-shell MP2](mp2.md#open-shell-uhf-mp2-example); create OH as in the HF guide. |
| Inspect and transform XYZ geometry | None | [Geometry](geometry.md); create an XYZ file. |
| Parameterized and batch studies | STO-3G and 6-31G | [Nickel studies](nickel-studies.md); obtain both bases. No separate Nickel executable is needed. |
| Create, inspect, verify and rerun bundles; reuse AO ERIs | STO-3G | [Portable bundles](portable-bundles.md); first prepare the H₂ input and basis. |
