//! Read h2.rustiq, run HF and MP2, then write the retained AO ERI to h2-copy.rustiq.
mod common;

use rustiq_core::{calculation::CalculationBuilder, config::Mp2Config, persistence::RustiQData};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (geometry, basis) = common::h2_inputs()?;
    let prepared = CalculationBuilder::new(&geometry, &basis)
        .with_mp2(Mp2Config::default())
        .prepare()?;
    let input = common::data_path("h2.rustiq")?;
    let mut data = RustiQData::open(input)?;
    let eri = data.take_compatible_eri(&prepared)?;
    let result = prepared.execute_with_eri(eri)?;
    let mut output = RustiQData::from_calculation(&prepared)?;
    let output_path = common::data_path("h2-copy.rustiq")?;
    output.write_with_eri(&output_path, result.hf.ao_eri())?;
    println!(
        "HF energy: {:.12} Eh",
        result.hf.summary().scf.electronic_energy
    );
    println!("Wrote {}", output_path.display());
    Ok(())
}
