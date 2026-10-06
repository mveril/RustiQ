//! CLI TOML frontend. Convert these representations to `rustiq_core::config`
//! before invoking scientific code; parsing is never needed for direct Rust use.
mod adapter;
pub mod basis;
pub mod cache;
mod diagnostics;
pub mod hf;
pub mod integrals;
pub mod method;
pub mod molecule;
pub mod mp2;
pub mod output;
pub mod parser;
pub mod random_config;
mod units;
pub mod validated;

use basis::BasisConfig;
use cache::CacheConfig;
use integrals::IntegralConfig;
use method::MethodConfig;
use molecule::MoleculeConfig;
use output::OutputConfig;
use toml_spanner::Toml;

#[derive(Debug, Toml)]
#[toml(Toml, recoverable)]
pub struct RunFile {
    #[toml(default, style = Header)]
    pub molecule: MoleculeConfig,
    #[toml(style = Header)]
    pub basis: BasisConfig,
    #[toml(default, style = Implicit)]
    pub method: MethodConfig,
    #[toml(default, style = Header)]
    pub integrals: IntegralConfig,
    #[toml(default, style = Header)]
    pub cache: CacheConfig,
    #[toml(default, style = Header)]
    pub output: OutputConfig,
}

#[cfg(test)]
mod sample_baseline;
