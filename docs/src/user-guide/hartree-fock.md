# Hartree–Fock calculations

Hartree–Fock (HF) is RustiQ's reference method for its calculations. It
approximates each electron as moving in an average field from the others. The
result is found iteratively by updating the electron density and solving the
effective one-electron equations until the density and energy are consistent.

RHF (restricted HF) uses shared spatial orbitals for paired alpha- and
beta-spin electrons. It is appropriate for a closed-shell state. UHF
(unrestricted HF) allows alpha and beta electrons to use different spatial
orbitals, which supports open-shell states. UHF is more flexible but its
wavefunction need not have a pure total spin.

## Select a method and starting guess

With `method = "Auto"`, RustiQ selects RHF for an even electron count and
singlet multiplicity, and UHF otherwise. In Nickel/TOML values these methods
are spelled `"Rhf"` and `"Uhf"`. Explicit RHF requires an even-electron
singlet; explicit UHF can also describe a singlet. This rule uses the requested charge
and multiplicity; it is not a ground-state search. The user remains responsible
for choosing the molecular state. Explicit `Rhf` or `Uhf` can also be requested
when compatible with the electron count.

This checked-in open-shell OH example uses neutral charge and doublet
multiplicity, so `Auto` resolves to UHF:

```toml
{{#include ../../../samples/oh/sto-3g/calculation.toml}}
```

Run it after importing STO-3G:

```sh
cargo run --locked -- basis import tests/data/sto-3g.json
cargo run --locked -- run samples/oh/sto-3g/calculation.toml
```

The open-shell CLI test and PySCF comparison are linked in
[scientific scope](../scientific-scope.md#reference-comparisons).

The checked-in H₂ input uses a core-Hamiltonian guess and resolves to RHF:

```toml
{{#include ../../../samples/h2/sto-3g/calculation.toml}}
```

An initial density guess starts the self-consistent iterations. Available
choices include core-Hamiltonian, one-electron, random, and zero guesses; the
default is core-Hamiltonian. A guess affects the starting path, not the
definition of the converged equations. Different guesses can lead to different
solutions in difficult cases.

## Iterations and convergence

SCF means self-consistent field, the repeated process of building a Fock
matrix (the effective one-electron operator) from the current density matrix,
solving for orbitals, then rebuilding the density. The atomic-orbital (AO)
basis consists of functions centered on atoms; molecular orbitals are
combinations of these functions. DIIS (direct inversion in
the iterative subspace) can extrapolate Fock matrices from previous iterations
to accelerate convergence. It is disabled by default; it changes the numerical
route, not the HF model.
The DIIS method is associated with Pulay's extrapolation approach
([Pulay, 1982](https://onlinelibrary.wiley.com/doi/10.1002/jcc.540030413)).

The default maximum is 100 iterations. The default `convergence_threshold` is
\\(10^{-8}\\). Both the absolute change in electronic energy between iterations
and the Frobenius norm of the AO commutator residual must fall below this
threshold. Its energy-change part is in Hartree. The residual is
\\(FPS-SPF\\), where \\(F\\), \\(P\\), and \\(S\\) are the Fock, density, and
overlap matrices; it measures inconsistency between the density and its Fock
operator. The Frobenius norm is the square root of the sum of squared matrix
entries. For UHF, alpha and beta (the two electron spin channels) residual norms
are combined as \\(\sqrt{r_\alpha^2+r_\beta^2}\\). The threshold is applied to
both diagnostics; it is not a bound on the final energy error.

The AO basis functions overlap. RustiQ diagonalizes their overlap matrix and
discards directions whose eigenvalues are at or below the configured relative
cutoff (default \\(10^{-8}\\) times the largest eigenvalue). The retained rank
reports the effective dimension used by HF. Rank reduction can change results;
inspect it when comparing calculations.

## Read the result carefully

The electronic HF energy includes electronic kinetic, electron–nuclear, and
electron–electron terms. Total HF energy adds nuclear repulsion. Energies are
in Hartree; input Angstrom coordinates are converted to Bohr internally.
Terminal energies are rounded for display. The H₂/STO-3G example is checked
against a PySCF 2.14.0 reference in the project comparison suite with an
absolute energy tolerance of \\(2\times10^{-10}\\) Hartree; the CLI test checks
the rounded value. See [reference comparison coverage](../scientific-scope.md#reference-comparisons)
and the [output guide](../getting-started/output.md).

For UHF, the output includes \\(\langle S^2\rangle\\) and its deviation from the
requested spin state's ideal value. A nonzero deviation is called spin
contamination. For a requested spin \\(S=(M-1)/2\\), where \\(M\\) is
multiplicity, the ideal expectation is \\(S(S+1)\\) in units of \\(\hbar^2\\).
For a doublet this is 0.75. Contamination is a diagnostic, not a correction
and not proof that UHF found the right state. A converged SCF solution can
still be a poor physical model or a local solution. HF uses one Slater
determinant and omits electron correlation beyond exchange;
small bases and strongly correlated systems need particular care. Read the
[scientific limitations](../scientific-scope.md) before interpreting results.
See the [glossary](../glossary.md) for shared terms.

The SCF criteria, overlap rank, and spin diagnostics are implemented in
[`scf.rs`](https://github.com/mveril/RustiQ/blob/main/crates/rustiq-core/src/hf/scf.rs),
[`orthogonalization.rs`](https://github.com/mveril/RustiQ/blob/main/crates/rustiq-core/src/hf/orthogonalization.rs),
and [`uhf.rs`](https://github.com/mveril/RustiQ/blob/main/crates/rustiq-core/src/hf/uhf.rs).
The checked-in H₂ and OH CLI/reference cases provide reproducible examples.

Next: [MP2](mp2.md).
