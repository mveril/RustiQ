# Molecular and basis-set data sources

This file records the provenance of molecular geometries and basis-set data
currently checked into RustiQ. Coordinates in the XYZ files are in ångströms.
For geometries without an external citation, no external provenance is recorded
in the repository; do not cite them as experimental or independently optimized
reference data.

## Molecular geometries

| Molecule | Repository files | Provenance and reuse notes |
| --- | --- | --- |
| Hydrogen, H₂ | `samples/h2/molecule.xyz`; `crates/rustiq-core/tests/data/samples/h2/molecule.xyz` | Project sample/test geometry, with atoms 0.74 Å apart. No external geometry source is recorded. The core fixture duplicates the sample. |
| Water, H₂O | `samples/h2o/h2o.xyz`; `crates/rustiq-core/tests/data/samples/h2o/h2o.xyz` | Project sample/test geometry. No external geometry source is recorded. The core fixture duplicates the sample. |
| Hydroxyl radical, OH | `samples/oh/oh.xyz`; `crates/rustiq-core/tests/data/samples/oh/oh.xyz` | Project sample/test geometry, with an O–H distance of 0.970 Å. No external geometry source is recorded. The core fixture duplicates the sample. |
| Lithium hydride, LiH | `samples/lih/lih.xyz` | Project-authored, fixed test geometry: Li at the origin and H at z = 1.60 Å. Chosen as a rounded equilibrium-like distance, consistent with the 1.5957 Å spectroscopic distance discussed in [Gottlieb and Weishäupl, *Comput. Theor. Chem.* 1007, 82–89 (2013)](https://doi.org/10.1016/j.comptc.2012.11.024), [author manuscript](https://pmc.ncbi.nlm.nih.gov/articles/PMC3564454/). It is not an experimental coordinate file or an optimization result. Neutral singlet; reused by the 6-31G RHF reference. |
| Carbon monoxide, CO | `samples/co/co.xyz` | Project-authored, fixed test geometry: C at the origin and O at z = 1.13 Å. A deliberately rounded equilibrium-like test distance; not an experimental coordinate file or an optimization result. Neutral singlet; the STO-3G RHF and RHF-MP2 references use this identical file. |
| Ethanol | `samples/ethanol/ethanol.xyz` | The file describes this as an approximate gas-phase geometry. No external source or calculation provenance is recorded; treat it as an illustrative project geometry, not as a sourced equilibrium structure. |
| Benzene | `samples/benzene/benzene.xyz` | 3D conformer identified by the file as PubChem CID 241 and retrieved through PubChem PUG REST. [PubChem record](https://pubchem.ncbi.nlm.nih.gov/compound/241); [PUG REST documentation](https://pubchem.ncbi.nlm.nih.gov/docs/pug-rest). PubChem's policy states that PubChem-generated information and participant-provided information are made available without cost and restriction, while warning that some contributed information may be copyrighted. The record labels this as a PubChem 3D conformer. |
| Cholesterol | `samples/cholesterol/cholesterol.xyz` | 3D conformer identified by the file as PubChem CID 5997. [PubChem record](https://pubchem.ncbi.nlm.nih.gov/compound/5997). PubChem's policy states that PubChem-generated information and participant-provided information are made available without cost and restriction, while warning that some contributed information may be copyrighted. The record labels this as a PubChem 3D conformer. |
| Anthracene | `samples/anthracene/anthracene.xyz` | PubChem 3D conformer for CID 8418, obtained as an SDF record through the [PubChem PUG REST 3D SDF endpoint](https://pubchem.ncbi.nlm.nih.gov/rest/pug/compound/cid/8418/SDF?record_type=3d); [PubChem compound record](https://pubchem.ncbi.nlm.nih.gov/compound/8418). The XYZ stores the 24 atom coordinates from that conformer in ångströms. PubChem policy says PubChem-generated and participant-provided information is made available without cost and restriction, but warns that some contributed information may be copyrighted; verify provenance-specific notices when present. |

PubChem's [data submission policy](https://pubchem.ncbi.nlm.nih.gov/pcfe/docs/markdown/data-submission-policy.md)
also places responsibility on users to respect restrictions that apply to
third-party contributed content. The PubChem attribution here describes the
geometry provenance recorded by the files; it is not a blanket clearance for
every item associated with a PubChem record.

## Geometry reuse review for the HF/MP2 extension

Reviewed on 2026-10-21. The new XYZ files are constructed by the project;
no third-party coordinate file, database table, spectrum, image, or article text
is redistributed. LiH uses a rounded test distance with literature attribution;
CO uses a project-selected rounded test distance. Existing water and OH
geometries are reused byte-for-byte, with their original unsourced status
preserved above. A new reference calculation does not establish external
provenance for an old geometry.

The [U.S. Copyright Office](https://www.copyright.gov/help/faq/faq-protect.html)
distinguishes facts from protected expression. This is not a blanket license
for database compilations or a determination of rights in every jurisdiction.
In particular, the [NIST SRD policy](https://www.nist.gov/open/license)
reserves compilation copyright under the Standard Reference Data Act, and the
[NIST Chemistry WebBook](https://webbook.nist.gov/chemistry/guide/) is SRD 69.
Its government origin must not be treated as automatic public-domain status.
The WebBook was consulted to check physical plausibility; its coordinate
exports and tables are not imported. The test geometries above intentionally
use project-selected rounded distances instead of copied database records.
No NIST license is claimed or granted by this repository.

## Locally stored basis sets

These are the basis-set datasets present in the repository's versioned
reference store. No other basis-set dataset is listed here. The reference
store README identifies them as unmodified files downloaded from the [Basis
Set Exchange (BSE)](https://www.basissetexchange.org/) using RustiQ. Their
embedded revision metadata is reproduced below.

| Basis set | Local source file | Embedded BSE revision metadata | Foundational reference |
| --- | --- | --- | --- |
| STO-3G | `tests/data/reference/RustiQ/basis_sets/sto-3g.json` | `Data from Gaussian09`, revision date `2018-06-19`, version `1` | Hehre, Stewart, and Pople, “Self-consistent molecular-orbital methods. I. Use of Gaussian expansions of Slater-type atomic orbitals,” *J. Chem. Phys.* 51, 2657 (1969), [DOI 10.1063/1.1672392](https://doi.org/10.1063/1.1672392). |
| 6-31G | `tests/data/reference/RustiQ/basis_sets/6-31g.json` | `Data from Gaussian 09/GAMESS`, revision date `2018-06-19`, version `1` | Hehre, Ditchfield, and Pople, “Self-Consistent Molecular Orbital Methods. XII. Further Extensions of Gaussian-Type Basis Sets for Use in Molecular Orbital Studies of Organic Molecules,” *J. Chem. Phys.* 56, 2257 (1972), [DOI 10.1063/1.1677527](https://doi.org/10.1063/1.1677527). |
| cc-pVDZ | `tests/data/reference/RustiQ/basis_sets/cc-pvdz.json` | `Data from ccRepo/Grant Hill`, revision date `2018-10-03`, version `1` | Dunning, “Gaussian basis sets for use in correlated molecular calculations. I. The atoms boron through neon and hydrogen,” *J. Chem. Phys.* 90, 1007 (1989), [DOI 10.1063/1.456153](https://doi.org/10.1063/1.456153). |

The BSE recommends citing its database papers when publishing work that uses
the exchange: Pritchard et al., [“A New Basis Set Exchange: An Open, Up-to-date
Resource for the Molecular Sciences Community”](https://doi.org/10.1021/acs.jcim.9b00725)
(2019), and, for the earlier database, Schuchardt et al., [“Basis Set Exchange:
A Community Database for Computational Sciences”](https://doi.org/10.1021/ci600510j)
(2007). The BSE documentation identifies the *software code* as BSD-3-Clause;
that statement alone does not establish a license for all contributed basis
data. The local JSON records do not state a separate data license. Retain their
attribution and review the applicable source terms before republishing these
datasets beyond the repository.

The HF/MP2 extension reuses the existing 6-31G lithium (`elements["3"]`)
and STO-3G carbon (`elements["6"]`) records. No BSE data was added,
modified, or downloaded for this extension. The reference environment copies
the existing versioned files into an isolated store. PySCF independently
normalizes their raw coefficients for the new cases. The upstream
[BSE software license](https://github.com/MolSSI-BSE/basis_set_exchange/blob/master/LICENSE)
was checked on 2026-10-21; its BSD redistribution conditions concern software
and do not resolve separate contributed-data rights. The attribution and
existing data-license limitation above continue to apply.

The repository also contains byte-identical copies of STO-3G at
`tests/data/sto-3g.json` and `crates/rustiq-core/tests/data/sto-3g.json`. Their
SHA-256 is `db3478afbe34866ed9ee8720e1bf8513d4de16dea22e568e7c1e838fd1231e40`.
They are project fixtures, distinct from the BSE snapshot above (SHA-256
`6e9edeb7ae5ee1c4155dbc84b8bad9b74f07077f8136c7531f86d7f0d358b638`); they
have no separate provenance metadata and are not described as a BSE download.

The project uses NIST's Computational Chemistry Comparison and Benchmark
Database (CCCBDB, SRD 101) as a possible reference resource only when its terms
permit. It is not the provenance of any geometry listed above, and no CCCBDB
geometry is bundled here. NIST identifies CCCBDB as SRD 101; consult the
[NIST license and terms](https://www.nist.gov/open/license) before any reuse.
