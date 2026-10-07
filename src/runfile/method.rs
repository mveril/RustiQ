use super::{hf::HfConfig, mp2::Mp2Config};

#[derive(Debug, Default, serde::Serialize)]
pub struct MethodConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hf: Option<HfConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mp2: Option<Mp2Config>,
}
