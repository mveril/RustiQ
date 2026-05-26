from pathlib import Path

from pyscf import gto, scf


HERE = Path(__file__).resolve().parent


def load_xyz_body(path):
    lines = path.read_text().splitlines()
    return "\n".join(lines[2:])


mol = gto.M(
    atom=load_xyz_body(HERE / "ethanol.xyz"),
    basis="sto-3g",
    charge=0,
    spin=0,
    unit="Angstrom",
    verbose=4,
)

mf = scf.RHF(mol)
mf.conv_tol = 1e-10
mf.max_cycle = 80
mf.diis_space = 8
energy = mf.kernel()

print(f"PySCF RHF/STO-3G total energy: {energy:.12f} Hartree")
print(f"PySCF converged: {mf.converged}")
print(f"PySCF SCF cycles: {mf.cycles}")
