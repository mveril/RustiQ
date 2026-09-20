from pathlib import Path

import pytest

from rustiq import BasisSet, Molecule, RHF, UHF


@pytest.fixture(scope="module")
def basis():
    path = Path(__file__).resolve().parents[3] / "tests/data/sto-3g.json"
    return BasisSet.from_file(path)


def hydrogen():
    return Molecule([("H", 0., 0., 0.), (1, 0., 0., .74)], units="angstrom")


@pytest.mark.parametrize("method", [RHF, UHF])
def test_hydrogen_hf_and_mp2(basis, method):
    hf = method(hydrogen(), basis, convergence_threshold=1e-10).run()
    assert hf.converged
    assert hf.total_energy == pytest.approx(-1.1167593074, abs=2e-8)
    assert hf.total_energy == pytest.approx(hf.electronic_energy + hf.nuclear_repulsion_energy)
    mp2 = hf.mp2()
    assert mp2.correlation_energy == pytest.approx(-0.0131380736, abs=2e-8)
    assert mp2.total_energy == pytest.approx(hf.total_energy + mp2.correlation_energy)
    assert hf.mp2().total_energy == mp2.total_energy
    with pytest.raises(ValueError, match="frozen"):
        hf.mp2(frozen_orbitals=2)


def test_open_shell_rejects_rhf(basis):
    molecule = Molecule([("H", 0., 0., 0.)], multiplicity=2)
    with pytest.raises(ValueError, match="closed-shell"):
        RHF(molecule, basis)
    hf = UHF(molecule, basis).run()
    assert hf.converged
    assert hf.total_energy == pytest.approx(-0.4665818496, abs=2e-8)
    assert hf.s_squared == pytest.approx(0.75)


def test_unconverged_hf_cannot_run_mp2(basis):
    hf = RHF(hydrogen(), basis, max_iterations=1).run()
    assert not hf.converged
    with pytest.raises(RuntimeError, match="converged"):
        hf.mp2()


@pytest.mark.parametrize("options", [
    {"max_iterations": 0}, {"convergence_threshold": 0},
    {"convergence_threshold": float("nan")}, {"diis_size": 1},
])
def test_invalid_options(basis, options):
    with pytest.raises(ValueError):
        RHF(hydrogen(), basis, **options)


def test_geometry_snapshot_and_units(basis):
    molecule = hydrogen()
    calculation = RHF(molecule, basis)
    molecule.geometry.rotate((0., 1., 0.), 0.5)
    molecule.geometry.translate((1., 2., 3.))
    reference = calculation.run().total_energy
    assert RHF(molecule, basis).run().total_energy == pytest.approx(reference, abs=1e-9)
    assert molecule.units == "angstrom"
    with pytest.raises(ValueError, match="units"):
        Molecule([("He", 0., 0., 0.)], units="invalid")


def test_basis_errors(tmp_path):
    with pytest.raises(FileNotFoundError):
        BasisSet.from_file(tmp_path / "missing.json")
    with pytest.raises(ValueError):
        BasisSet.from_json("{}")
