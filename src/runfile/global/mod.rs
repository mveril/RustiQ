pub mod molecule_config;
use molecule_config::MoleculeConfig;

#[allow(dead_code)]
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Global {
    pub(crate) basis: String,
    #[serde(default)]
    pub(crate) molecule: MoleculeConfig,
}
