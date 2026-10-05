//! Run HF using the AO ERI stored in h2.rustiq.
mod common;

use rustiq_core::{calculation::CalculationBuilder, persistence::RustiQData};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (geometry, basis) = common::h2_inputs()?;
    let prepared = CalculationBuilder::new(&geometry, &basis).prepare()?;
    let input = common::data_path("h2.rustiq")?;
    let mut data = RustiQData::open(input)?;
    let eri = data.take_compatible_eri(&prepared)?;
    let hf = prepared.run_hf_with_eri(eri)?;
    println!("HF energy: {:.12} Eh", hf.summary().scf.electronic_energy);
    Ok(())
}
