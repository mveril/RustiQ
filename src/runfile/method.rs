use toml_spanner::Toml;

use super::{hf::HfConfig, mp2::Mp2Config};

#[derive(Debug, Default, Toml)]
#[toml(Toml, recoverable)]
pub struct MethodConfig {
    #[toml(default, style = Header)]
    pub hf: Option<HfConfig>,
    #[toml(default, style = Header)]
    pub mp2: Option<Mp2Config>,
}
