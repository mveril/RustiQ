"""Independent HF energy profiles for production RHF/UHF damping steps."""

import json
import os

import numpy as np
import pytest

pytest.importorskip("pyscf", reason="damping references require PySCF")

from pyscf import gto, lib, scf
from scipy.optimize import minimize_scalar

from compare_pyscf import CASES, bse_basis, run_command

pytestmark = pytest.mark.requires_rust


@pytest.fixture(scope="module")
def damping_results(tmp_path_factory):
    output = tmp_path_factory.mktemp("damping-reference")
    run_command(
        ["cargo", "test", "-p", "rustiq-chem-core", "--no-default-features", "--lib", "damping_reference"],
        env=dict(os.environ, RUSTIQ_DAMPING_OUTPUT=str(output)),
    )
    return {path.stem: json.loads(path.read_text()) for path in output.glob("*.json")}


@pytest.mark.parametrize(
    "name",
    ["h2-rhf-ordinary", "h2-uhf-ordinary", "h2-uhf-diis", "oh-uhf", "h2-dianion-rhf", "h2-dianion-uhf"],
)
def test_rhf_uhf_damping_matches_independent_pyscf_energy_profile(name, damping_results):
    lib.num_threads(1)
    actual = damping_results[name]
    system = actual["system"]
    is_uhf = actual["method"] == "uhf"
    if system == "oh":
        atoms = [("O", (0, 0, 0)), ("H", (0, 0, 0.97 / 0.529177210903))]
        charge, spin = 0, 1
        # RustiQ uses pz,py,px; libcint uses px,py,pz for the oxygen shell.
        inverse = np.argsort([0, 1, 4, 3, 2, 5])
    else:
        distance = 0.74 / 0.529177210903 if system == "h2-dianion" else 6.0
        atoms = [("H", (0, 0, -distance / 2)), ("H", (0, 0, distance / 2))]
        charge, spin = (-2 if system == "h2-dianion" else 0), 0
        inverse = np.arange(2)
    # The OH fixture selects both H and O from the same checked-in STO-3G file.
    case = next(case for case in CASES if case.name == "oh-sto-3g-uhf")
    mol = gto.M(atom=atoms, unit="Bohr", basis=bse_basis(case), charge=charge, spin=spin, verbose=0)
    mf = scf.UHF(mol) if is_uhf else scf.RHF(mol)
    overlap, hcore = mf.get_ovlp(), mf.get_hcore()

    def matrices(key):
        channels = [
            np.array(values).reshape((actual["nao"], actual["nao"]), order="F")[np.ix_(inverse, inverse)]
            for values in actual[key]
        ]
        return np.array(channels) if is_uhf else channels[0]

    def physical_fock(density):
        return hcore + mf.get_veff(dm=density)

    def energy(density):
        return mf.energy_elec(dm=density, h1e=hcore, vhf=mf.get_veff(dm=density))[0]

    density = matrices("density")
    fock = physical_fock(density)
    np.testing.assert_allclose(matrices("fock"), fock, atol=1e-9, rtol=0)
    assert actual["energy"] == pytest.approx(energy(density), abs=1e-9, rel=0)
    populations = np.einsum("...ij,ji->...", density, overlap)
    expected = mol.nelec if is_uhf else mol.nelectron
    np.testing.assert_allclose(populations, expected, atol=1e-10, rtol=0)

    if system == "h2-dianion":
        # The unconstrained minimum from Zero would lower energy by losing N.
        # This is a finite-basis regression; it asserts no physical binding.
        assert actual["energy"] > 0.2
        invalid = minimize_scalar(lambda t: energy(t * density), bounds=(0, 1), method="bounded")
        assert 0.4 < invalid.x < 0.5
        assert np.all(invalid.x * populations < expected)
        return

    residual = fock @ density @ overlap - overlap @ density @ fock
    assert actual["residual"] == pytest.approx(np.linalg.norm(residual), abs=1e-9, rel=0)
    previous, candidate = matrices("previous"), matrices("candidate")
    step = candidate - previous
    previous_energy, candidate_energy = energy(previous), energy(candidate)
    assert actual["previous_energy"] == pytest.approx(previous_energy, abs=1e-9, rel=0)
    assert actual["candidate_energy"] == pytest.approx(candidate_energy, abs=1e-9, rel=0)
    slope = np.sum(step * physical_fock(previous))
    curvature = 0.5 * np.sum(step * (physical_fock(candidate) - physical_fock(previous)))
    for fraction in np.linspace(0, 1, 101):
        trial = previous + fraction * step
        trial_energy = energy(trial)
        assert trial_energy == pytest.approx(previous_energy + slope * fraction + curvature * fraction**2, abs=1e-10, rel=0)
        assert actual["energy"] <= trial_energy + 1e-9
        np.testing.assert_allclose(np.einsum("...ij,ji->...", trial, overlap), expected, atol=1e-10, rtol=0)
    # The optimizer samples rebuilt PySCF energies, independent of Rust's formula.
    minimum = minimize_scalar(lambda t: energy(previous + t * step), bounds=(0, 1), method="bounded")
    assert actual["energy"] <= min(previous_energy, candidate_energy, minimum.fun) + 1e-9
    if system == "h2":
        assert candidate_energy > previous_energy + 0.05
        assert actual["fraction"] == pytest.approx(minimum.x, abs=1e-7, rel=0)
        np.testing.assert_allclose(density, previous + actual["fraction"] * step, atol=1e-12, rtol=0)
        determinant_density = density[0] if is_uhf else density / 2
        assert np.linalg.norm(determinant_density @ overlap @ determinant_density - determinant_density) > 0.1
