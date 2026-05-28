pub mod molecule_config;
use molecule_config::MoleculeConfig;

use toml_spanner::Toml;

#[derive(Debug, Toml)]
#[toml(Toml, recoverable)]
pub(crate) struct Global {
    pub(crate) basis: String,
    #[toml(default)]
    pub(crate) molecule: MoleculeConfig,
}
