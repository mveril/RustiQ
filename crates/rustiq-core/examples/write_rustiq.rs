//! Run HF and write its retained AO ERI to h2.rustiq.
mod common;

use rustiq_core::calculation::CalculationBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (geometry, basis) = common::h2_inputs()?;
    let prepared = CalculationBuilder::new(&geometry, &basis).prepare()?;
    let hf = prepared.run_hf()?;
    let mut data = rustiq_core::persistence::RustiQData::from_calculation(&prepared)?;
    let output = common::data_path("h2.rustiq")?;
    data.write_with_eri(&output, hf.ao_eri())?;
    println!("Wrote {}", output.display());
    Ok(())
}
