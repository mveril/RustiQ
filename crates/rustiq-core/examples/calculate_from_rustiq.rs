//! Restore a calculation and its AO ERI from a single `.rustiq` archive.
//! Create the default archive with `cargo run -p rustiq-core --example write_rustiq`,
//! then run this example with `cargo run -p rustiq-core --example calculate_from_rustiq`.
#[allow(
    dead_code,
    reason = "Retained helper supports scientific tests and benchmarks"
)]
mod common;

use rustiq_core::{calculation::CalculationExecution, persistence::RustiQData};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = common::data_path("h2.rustiq")?;
    let data = RustiQData::open(&input)?;
    let prepared = data.prepare_calculation()?;
    let result = prepared.execute()?;
    println!("Retained AO ERI values: {}", result.hf.ao_eri().len());

    println!(
        "HF energy: {:.12} Eh",
        result.hf.summary().scf.electronic_energy
    );
    if let Some(mp2) = result.mp2 {
        println!("MP2 correlation energy: {:.12} Eh", mp2.correlation_energy);
    }
    Ok(())
}
