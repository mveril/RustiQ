//! Persist and inspect converged orbitals using the bundled offline H2 fixture.
use rustiq_core::{
    persistence::{ConvergedHf, ConvergedHfArtifact, HfOrbitals, RustiQData},
    prelude::*,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let geometry = Geometry::from_source("h2.xyz", "2\nH2 in Bohr\nH 0 0 0\nH 1.4 0 0\n")?;
    let basis = BasisFile::from_reader(
        &include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/data/sto-3g.json"
        ))[..],
    )?;
    let prepared = CalculationBuilder::new(&geometry, &basis).prepare()?;
    let solution = prepared.run_hf()?.into_converged()?;
    let mut data = RustiQData::from_calculation(&prepared)?;
    data.set::<ConvergedHfArtifact>(ConvergedHf::from_solution(&solution)?)?;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("h2.rustiq");
    // Optionally include the existing AO ERI without copying its representation.
    data.write_with_eri(&path, solution.ao_eri())?;
    let mut restored = RustiQData::open(path)?;
    if !restored.hf_is_compatible(&prepared) {
        return Err("incompatible HF context".into());
    }
    let orbitals = restored
        .get::<ConvergedHfArtifact>()?
        .ok_or("HF artifact missing")?;
    if let HfOrbitals::Rhf { shared } = orbitals.orbitals() {
        if shared.occupied() != 1 {
            return Err("unexpected occupation".into());
        }
        if shared.coefficients()
            != solution
                .as_restricted()
                .ok_or("expected RHF")?
                .mo_coefficients()
                .as_slice()
        {
            return Err("coefficients changed".into());
        }
    }
    // Reading orbitals does not reconstruct a solution or run MP2.
    Ok(())
}
