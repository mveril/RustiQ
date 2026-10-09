# Interpreting output

The report shows original sources, normalized requested options, resolved
scientific options, and results. These views explain defaults and conversions;
they represent the same calculation.

The first sample resolves to neutral singlet H₂, RHF, STO-3G, and two basis
functions. Input coordinates are Angstrom; resolved coordinates are Bohr.
Energies are in Hartree.

- **Electronic energy:** electronic kinetic energy, electron–nuclear attraction,
  and electron–electron interaction at the HF level.
- **Nuclear repulsion energy:** repulsion between the fixed nuclei.
- **Total HF energy:** electronic energy plus nuclear repulsion.
- **SCF convergence:** the iterative solution met its configured criterion.
  This does not establish basis completeness or accuracy against experiment.

The [first-calculation test](first-calculation.md) checks the rounded total
energy of `-1.116759` Hartree. The independent comparison uses full precision.

MP2 adds a correlation energy to a converged HF reference. This first sample
does not request MP2. The
[checked-in MP2 sample](https://github.com/mveril/RustiQ/blob/main/samples/h2/sto-3g/mp2_calculation.toml)
and `h2-sto-3g-rhf-mp2` PySCF comparison are executable evidence for a future
dedicated guide.

A successful small-basis H₂ test does not validate every molecule, basis,
open-shell state, or chemical prediction. Review the
[reference test coverage](https://github.com/mveril/RustiQ/blob/main/tools/reference/README.md)
and [research limitations](https://github.com/mveril/RustiQ/blob/main/ROADMAP.md).
