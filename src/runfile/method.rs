use toml_spanner::Toml;

use super::{hf::HfConfig, mp2::Mp2Config};

#[derive(Debug, Default, Toml)]
#[toml(Toml, recoverable)]
pub struct MethodConfig {
    #[toml(default)]
    pub hf: Option<HfConfig>,
    #[toml(default)]
    pub mp2: Option<Mp2Config>,
}
