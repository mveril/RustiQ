# Scientific scope and limitations

RustiQ is experimental software for molecular electronic energies at a fixed
geometry. A successful run or reference comparison does not establish
research-grade correctness. Independently validate the method, basis, molecular
state, convergence, and quantities relevant to your research against established
packages before drawing chemical conclusions.

## Implemented scope

Hartree–Fock (HF) is available as restricted HF (RHF) for closed shells and
unrestricted HF (UHF) with separate alpha and beta orbitals for open shells.
Automatic selection resolves the method from the molecular state. DIIS
accelerates the iterative SCF solution; core-Hamiltonian and randomized density
guesses are available. MP2 adds a correlation correction to converged RHF or
UHF orbitals, with optional frozen occupied orbitals and a blocked
transformation governed by a workspace budget.

Calculations use XYZ geometries with Angstrom or Bohr input, charge and spin
multiplicity, Gaussian basis functions, one-electron integrals, and compact
stored electron-repulsion integrals (ERIs). Schwarz screening can skip small
ERIs, and overlap orthogonalization removes near-dependent directions according
to a configured threshold. These thresholds can affect energies and convergence;
convergence alone does not establish their suitability for a particular system.
Increasing `integrals.schwarz_threshold` skips more small integrals at a
potential accuracy cost; setting it to zero disables Schwarz screening.
The authoritative default is in the [Nickel schema](reference/index.md).

Gaussian Cartesian and spherical functions are implemented, including spherical
d functions. Spherical angular momentum above d and effective core potentials
(ECPs) are rejected by the current basis loader. The relevant implementation
is linked below for inspection.

[Basis loader source on GitHub](https://github.com/mveril/RustiQ/blob/main/crates/rustiq-core/src/basis/gaussian/basis.rs)
Downloading a basis file does not guarantee that its contents are supported or
numerically validated. Geometry inspection and coordinate transformations are
available; these operations do not optimize the geometry's energy.
See the [glossary](glossary.md) for terminology and the
[workflow notes](reference/existing-workflows.md) for preserved usage guidance.

## Reference comparisons

The book's [first H₂ calculation](getting-started/first-calculation.md)
provides a reproducible entry point to independent PySCF comparisons with
explicit tolerances. The comparison instructions and case definitions are
linked below. They cover
H₂ and water in STO-3G, 6-31G, and cc-pVDZ, open-shell H₂⁺ and OH, and selected
RHF/UHF MP2 cases. Additional cases exercise frozen occupied orbitals and
small versus larger MP2 workspace budgets. For OH/STO-3G, the UHF-only sample
uses a numerical SCF threshold of `1e-5` in both RustiQ and PySCF, while the
UHF-MP2 sample uses `1e-10` in both. Matching the numerical thresholds does
not make the two codes' SCF stopping criteria identical: RustiQ checks an
energy change and an AO commutator residual, while PySCF uses its own
convergence tests. Compare converged energies and spin diagnostics rather than
assuming equivalent accuracy from a matching input number. The
[first H₂ calculation](getting-started/first-calculation.md) identifies its
CLI test and full-precision independent comparison.

This is a useful, limited reference suite. Its tolerances are case-specific;
the H₂/STO-3G tolerance is not a universal accuracy guarantee. It does not
systematically span elements, contraction conventions, angular momenta,
charges, spin states, difficult SCF solutions, or large systems. Agreement in
selected energies can leave errors in untested quantities or shared assumptions
undetected. It also does not prove basis completeness, adequacy of HF/MP2 for a
chemical problem, or agreement with experiment. UHF spin contamination is an
additional physical concern even for converged solutions.

## Physical limits of HF and MP2

HF represents the state with a single Slater determinant and omits electron
correlation beyond exchange. Convergence establishes a self-consistent solution
within the selected orbital space; it does not establish that the solution is
the lowest-energy HF state. Different initial guesses can find different
solutions. UHF can lower its energy by breaking spin symmetry, so inspect
\\(\langle S^2\rangle\\) and spin contamination as well as the energy.

Bond stretching, dissociation, and near-degenerate states can require several
important electronic configurations. A converged single-determinant reference
can then be physically inadequate. MP2 is a perturbative correction to that
reference, not a general remedy for strong correlation. Its energy terms contain
denominators \\(\epsilon_i + \epsilon_j - \epsilon_a - \epsilon_b\\), with
occupied indices \\(i,j\\) and virtual indices \\(a,b\\). Small denominators can
produce excessively large corrections. Lee and Head-Gordon discuss these
denominator problems and bond-breaking examples in their
[study of regularized MP2](https://arxiv.org/abs/1807.06185).

RustiQ rejects non-finite denominators and absolute denominator values at or
below `1e-12` Hartree. This numerical guard is not a physical suitability test:
passing it does not establish that the perturbation expansion is reliable.
Agreement with another HF/MP2 implementation tests numerical consistency under
matched conventions; judging a chemical prediction also requires an appropriate
method, basis, and molecular state.

## Major limitations

The current calculation methods do not provide DFT, analytic gradients,
geometry optimization, vibrational frequencies, or post-HF methods beyond MP2.
The molecular input workflow uses XYZ rather than a broad set of chemistry
formats. General research use needs broader independent numerical validation,
scientific convention documentation, and performance evidence.

Memory management and screening already exist, but compact ERI storage can
still grow rapidly with basis size. The MP2 workspace budget is not a bound on
total process memory. Larger checked-in molecules are examples, not evidence
of robust or competitive performance at scale.

The [authoritative references](reference/index.md) page links to the roadmap
and issues, which own future plans. Detailed method derivations and broader
validation methodology belong to later scientific documentation; this page
describes current boundaries.

[Reference comparison instructions on GitHub](https://github.com/mveril/RustiQ/blob/main/tools/reference/README.md)

[Reference case definitions on GitHub](https://github.com/mveril/RustiQ/blob/main/tools/reference/compare_pyscf.py)

[Project roadmap on GitHub](https://github.com/mveril/RustiQ/blob/main/ROADMAP.md)
