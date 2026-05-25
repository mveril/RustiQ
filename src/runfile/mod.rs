pub mod global;
pub mod hf;
use global::Global;
pub mod random_config;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct RunFile {
    pub(crate) global: Global,
    pub(crate) hf: Option<hf::HfConfig>,
}
