# Python calculations

Build/install this workspace member with maturin, then select that Python
environment in VS Code. The wheel includes type stubs for completion.

```python
from rustiq import BasisSet, Molecule, RHF, UHF

basis = BasisSet.from_file("tests/data/sto-3g.json")
h2 = Molecule(
    [("H", 0.0, 0.0, 0.0), (1, 0.0, 0.0, 0.74)],
    units="angstrom",
)
hf = RHF(h2, basis, convergence_threshold=1e-10).run()
if hf.converged:
    mp2 = hf.mp2(frozen_orbitals=0)
    print(hf.total_energy, mp2.correlation_energy, mp2.total_energy)

oh = Molecule(
    [("O", 0.0, 0.0, 0.0), ("H", 0.0, 0.0, 0.97)],
    multiplicity=2, units="angstrom",
)
uhf = UHF(oh, basis).run()
if uhf.converged:
    print(uhf.s_squared, uhf.spin_contamination)
    print(uhf.mp2().total_energy)
```

Basis files use the Basis Set Exchange JSON format. Loading is explicit and
does not download data. Coordinates default to Bohr; energy results are Hartree.
Charge defaults to zero and multiplicity to one. Rotation angles are radians
and axes/translations are tuples of three numbers.

RHF and UHF take a snapshot of the molecule at construction. Each `run()`
starts a fresh SCF calculation, releasing the Python GIL during computation.
An exhausted iteration limit returns `converged=False` with the final metrics;
`mp2()` rejects this state. Other setup/execution errors raise Python exceptions.
MP2 reuses the retained HF orbitals and integrals, without rerunning SCF.
The frozen count excludes that many occupied spatial orbitals in RHF, or that
many occupied orbitals from each spin in UHF.

Total MP2 energy includes nuclear repulsion; correlation energy is the
correction alone.

Run `pytest crates/rustiq-python/tests` using the environment containing the
newly built extension. The numerical tests use the repository STO-3G fixture.
