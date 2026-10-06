//! CLI configuration frontend. Production TOML resolves to syntax-neutral DTOs
//! before conversion to core types; embedded Nickel owns TOML defaults and validation.
mod adapter;
pub mod basis;
pub mod cache;
mod diagnostics;
pub mod hf;
pub mod integrals;
pub mod method;
pub mod molecule;
pub mod mp2;
mod nickel;
pub mod output;
pub mod parser;
pub mod random_config;
pub mod resolved;
mod source_map;
mod units;
pub mod validated;

use basis::BasisConfig;
use cache::CacheConfig;
use integrals::IntegralConfig;
use method::MethodConfig;
use molecule::MoleculeConfig;
use output::OutputConfig;

#[derive(Debug, serde::Serialize)]
pub struct RunFile {
    pub molecule: MoleculeConfig,
    pub basis: BasisConfig,
    pub method: MethodConfig,
    pub integrals: IntegralConfig,
    pub cache: CacheConfig,
    pub output: OutputConfig,
}

#[cfg(test)]
mod sample_baseline;

#[cfg(test)]
fn parse_section(section: &str, source: &str) -> miette::Result<RunFile> {
    parser::parse_runfile(
        "test.toml",
        &format!("[basis]\nname = 'sto-3g'\n[{section}]\n{source}"),
    )
    .map(|parsed| parsed.runfile)
}
