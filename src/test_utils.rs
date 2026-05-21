use std::fs::File;

use crate::{
    basis::{basisfile::BasisFile, gaussian::basis::Basis},
    hf::{density_guess::one_electron::OneElectron, scf::ScfCalculation},
    molecules::{geometry::Geometry, molecule::Molecule},
};

pub(crate) fn load_minimal_basis_file() -> BasisFile {
    let file = File::open("tests/data/sto-3g.json").unwrap();
    serde_json::from_reader(file).unwrap()
}

pub(crate) fn load_sample_geometry(path: &str) -> Geometry {
    let file = File::open(path).unwrap();
    Geometry::from_file(file, None, None).unwrap()
}

pub(crate) fn load_sto3g_basis(geometry: &Geometry) -> Basis {
    let basis_file = load_minimal_basis_file();
    Basis::load(&basis_file, geometry)
}

pub(crate) fn new_one_electron_scf<'a>(
    molecule: &'a Molecule,
    basis: &'a Basis,
    max_iterations: usize,
    convergence_threshold: f64,
) -> ScfCalculation<'a> {
    ScfCalculation::new(
        molecule,
        basis,
        max_iterations,
        convergence_threshold,
        Box::new(OneElectron),
    )
}

pub(crate) struct ScfReferenceResult {
    pub(crate) electronic_energy: f64,
    pub(crate) nuclear_repulsion_energy: f64,
    pub(crate) total_energy: f64,
}

pub(crate) fn run_sto3g_scf_for_sample(path: &str) -> ScfReferenceResult {
    let geometry = load_sample_geometry(path);
    let basis = load_sto3g_basis(&geometry);
    let molecule = Molecule::from(geometry);
    let mut scf = new_one_electron_scf(&molecule, &basis, 100, 1e-8);

    let result = scf.run();

    ScfReferenceResult {
        electronic_energy: result.electronic_energy,
        nuclear_repulsion_energy: result.nuclear_repulsion_energy,
        total_energy: result.total_energy,
    }
}
