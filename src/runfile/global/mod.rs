pub mod molecule_config;
use molecule_config::MoleculeConfig;

use crate::molecules::units::Units;
#[allow(dead_code)]
use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Global {
    pub(crate) basis: String,
    #[serde(default)]
    pub(crate) molecule: MoleculeConfig,
}
