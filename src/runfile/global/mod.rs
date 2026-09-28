pub mod molecule_config;
use molecule_config::MoleculeConfig;

use toml_spanner::Toml;

#[derive(Debug, Toml)]
#[toml(Toml, recoverable)]
pub struct Global {
    pub basis: String,
    #[toml(default)]
    pub molecule: MoleculeConfig,
}
