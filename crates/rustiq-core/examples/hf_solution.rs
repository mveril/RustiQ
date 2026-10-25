//! Inspect RHF/UHF output and retry MP2 after an input error, entirely offline.
use std::{error::Error, num::NonZeroU8};

use rustiq_core::{
    calculation::{AnyHfSolution, CalculationError, Mp2Error},
    prelude::*,
};

fn main() -> Result<(), Box<dyn Error>> {
    let basis = BasisFile::from_reader(
        &include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/data/sto-3g.json"
        ))[..],
    )?;
    let h2 = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/samples/h2/molecule.xyz"
    ));
    let oh = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/samples/oh/oh.xyz"
    ));

    // Auto selects RHF for the H2 singlet and UHF for the OH doublet.
    run_case(h2, NonZeroU8::MIN, &basis)?;
    run_case(oh, NonZeroU8::new(2).ok_or("invalid multiplicity")?, &basis)?;
    Ok(())
}

fn run_case(xyz: &[u8], multiplicity: NonZeroU8, basis: &BasisFile) -> Result<(), Box<dyn Error>> {
    let geometry = Geometry::from_reader(xyz)?;
    let prepared = CalculationBuilder::new(&geometry, basis)
        .with_molecule_config(MoleculeConfig {
            units: Units::Angstrom,
            multiplicity: multiplicity.into(),
            ..Default::default()
        })
        .prepare()?;

    // Failure to converge propagates an error retaining the diagnostic HF output.
    let hf = prepared.run_hf()?.into_converged()?;
    if let Some(rhf) = hf.as_restricted() {
        println!(
            "RHF doubly occupied spatial orbitals: {}",
            rhf.occupied_orbitals()
        );
    }
    if let Some(uhf) = hf.as_unrestricted() {
        let occupied = uhf.occupied_orbitals();
        println!(
            "UHF occupied alpha/beta orbitals: {}/{}",
            occupied.alpha(),
            occupied.beta()
        );
    }
    println!(
        "{} HF total energy: {:.12} Eh",
        hf.method(),
        hf.summary().scf.total_energy
    );

    // Deliberately freeze more orbitals than are occupied in either example.
    let method = hf.method();
    let config = Mp2Config {
        frozen_orbitals: 99.into(),
        ..Default::default()
    };
    let Err(error) = (match &hf {
        AnyHfSolution::Restricted(hf) => hf.mp2(config),
        AnyHfSolution::Unrestricted(hf) => hf.mp2(config),
    }) else {
        return Err("the deliberately invalid MP2 configuration unexpectedly succeeded".into());
    };
    if !matches!(
        error.cause(),
        CalculationError::Mp2 {
            error: Mp2Error::InvalidFrozenOrbitalCount { .. },
            ..
        }
    ) {
        return Err(error.into());
    }
    println!("{} expected MP2 input error: {}", method, error.cause());

    // Drop the original handle: the error still owns the completed HF output.
    drop(hf);
    let outcome = error
        .into_hf()
        .ok_or("MP2 error did not retain HF output")?;
    // AnyHfSolution defaults to Converged; try_into checks the retained outcome.
    let recovered: AnyHfSolution = outcome.try_into()?;
    let mp2 = match recovered {
        AnyHfSolution::Restricted(hf) => hf.mp2(Mp2Config::default())?,
        AnyHfSolution::Unrestricted(hf) => hf.mp2(Mp2Config::default())?,
    };
    println!(
        "{} MP2 correlation energy after retry: {:.12} Eh",
        method, mp2.correlation_energy
    );
    Ok(())
}
