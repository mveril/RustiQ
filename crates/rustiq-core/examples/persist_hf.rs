//! Persist, inspect, and restore converged HF using the bundled offline H2 fixture.
use rustiq_core::{
    persistence::{ConvergedHfArtifact, HfOrbitals, RustiQData},
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
    data.set::<ConvergedHfArtifact>(
        &solution
            .as_restricted()
            .ok_or("expected RHF")?
            .as_scf()
            .to_artifact()?,
    )?;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("h2.rustiq");
    // Include the existing AO ERIs required to restore an executable solution.
    data.write_with_eri(&path, solution.ao_eri())?;
    let restored = RustiQData::open(path)?;
    if !restored.hf_is_compatible(&prepared) {
        return Err("incompatible HF context".into());
    }
    let orbitals = restored
        .get::<ConvergedHfArtifact>()?
        .ok_or("HF artifact missing")?;
    if let HfOrbitals::Rhf(rhf) = orbitals.orbitals() {
        if rhf.shared.occupied() != 1 {
            return Err("unexpected occupation".into());
        }
        if rhf.shared.coefficients()
            != solution
                .as_restricted()
                .ok_or("expected RHF")?
                .mo_coefficients()
                .as_slice()
        {
            return Err("coefficients changed".into());
        }
    }
    let hf: RhfSolution = restored.restore()?;
    let reference = solution.as_restricted().ok_or("expected RHF")?;
    let config = Mp2Config::default();
    let expected = reference.mp2(config)?.correlation_energy;
    let actual = hf.mp2(config)?.correlation_energy;
    if (actual - expected).abs() > 1e-12 {
        return Err("restored MP2 correlation energy changed".into());
    }
    Ok(())
}
