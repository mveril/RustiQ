//! Run HF and MP2 directly from the H2 sample inputs.
mod common;

use rustiq_core::{
    calculation::{CalculationBuilder, CalculationExecution},
    config::Mp2Config,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (geometry, basis) = common::h2_inputs()?;
    let prepared = CalculationBuilder::new(&geometry, &basis)
        .with_mp2(Mp2Config::default())
        .prepare()?;
    let result = prepared.execute()?;
    println!(
        "HF energy: {:.12} Eh",
        result.hf.summary().scf.electronic_energy
    );
    if let Some(mp2) = result.mp2 {
        println!("MP2 correlation energy: {:.12} Eh", mp2.correlation_energy);
    }
    Ok(())
}
