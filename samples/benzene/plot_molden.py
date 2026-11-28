"""Plot a molecular orbital in a plane parallel to the benzene ring."""

import argparse

import matplotlib
import numpy as np
from pyscf.tools import molden

matplotlib.use("Agg")
import matplotlib.pyplot as plt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("molden")
    parser.add_argument("output")
    args = parser.parse_args()

    molecule, energies, coefficients, occupations, *_ = molden.load(args.molden)
    homo = np.flatnonzero(occupations > 0)[-1]
    axis = np.linspace(-7, 7, 201)
    x, y = np.meshgrid(axis, axis)
    points = np.column_stack((x.ravel(), y.ravel(), np.full(x.size, 1.0)))
    atomic_orbitals = molecule.eval_gto("GTOval", points)
    orbital = (atomic_orbitals @ coefficients[:, homo]).reshape(x.shape)
    limit = np.max(np.abs(orbital))

    figure, plot = plt.subplots(figsize=(5, 4.5), layout="constrained")
    contours = plot.contourf(
        x, y, orbital, levels=np.linspace(-limit, limit, 25), cmap="RdBu_r"
    )
    coords = molecule.atom_coords()
    plot.scatter(coords[:, 0], coords[:, 1], c="black", s=8)
    plot.set(
        aspect="equal",
        xlabel="x (Bohr)",
        ylabel="y (Bohr)",
        title=f"Benzene HOMO at z = 1 Bohr\n{energies[homo]:.3f} Hartree",
    )
    figure.colorbar(contours, ax=plot, label="Orbital amplitude (Bohr⁻³ᐟ²)")
    figure.savefig(args.output, dpi=150)


if __name__ == "__main__":
    main()
