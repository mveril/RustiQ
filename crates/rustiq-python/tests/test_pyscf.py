"""Independent open-shell and frozen-core references (requires PySCF)."""
from pathlib import Path

import pytest

from rustiq import BasisSet, Molecule, RHF, UHF

pyscf = pytest.importorskip("pyscf")


@pytest.mark.parametrize("restricted", [True, False])
@pytest.mark.parametrize("frozen", [0, 1])
def test_mp2_against_pyscf(restricted, frozen):
    atoms = [("O", (0., 0., 0.)), ("H", (0., 0., .97))]
    if restricted:
        atoms.append(("H", (.92, 0., -.24)))
    reference_molecule = pyscf.gto.M(
        atom=atoms, basis="sto-3g", unit="angstrom",
        spin=0 if restricted else 1, verbose=0,
    )
    pyscf.lib.num_threads(1)
    reference_hf = (pyscf.scf.RHF if restricted else pyscf.scf.UHF)(reference_molecule)
    reference_hf.conv_tol = 1e-11
    reference_hf.kernel()
    assert reference_hf.converged
    reference_mp2 = reference_hf.MP2(frozen=frozen).run()
    path = Path(__file__).resolve().parents[3] / "tests/data/sto-3g.json"
    molecule = Molecule(
        [(symbol, *position) for symbol, position in atoms],
        multiplicity=1 if restricted else 2, units="angstrom",
    )
    hf = (RHF if restricted else UHF)(
        molecule, BasisSet.from_file(path), convergence_threshold=1e-10,
    ).run()
    assert hf.converged
    assert hf.total_energy == pytest.approx(reference_hf.e_tot, abs=2e-7)
    assert hf.mp2(frozen_orbitals=frozen).correlation_energy == pytest.approx(
        reference_mp2.e_corr, abs=2e-7
    )
