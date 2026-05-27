from pyscf import gto, scf

mol = gto.M(
    atom="""
    O 0.0 0.0 0.000
    H 0.0 0.0 0.970
    """,
    basis="sto-3g",
    unit="Angstrom",
    charge=0,
    spin=1,
    verbose=0,
)

mf = scf.UHF(mol)
total_energy = mf.kernel()
electronic_energy = total_energy - mol.energy_nuc()

print(f"PySCF UHF/STO-3G electronic energy: {electronic_energy:.12f} Hartree")
print(f"PySCF UHF/STO-3G nuclear repulsion energy: {mol.energy_nuc():.12f} Hartree")
print(f"PySCF UHF/STO-3G total energy: {total_energy:.12f} Hartree")
print(f"PySCF converged: {mf.converged}")
print(f"PySCF SCF cycles: {mf.cycles}")
