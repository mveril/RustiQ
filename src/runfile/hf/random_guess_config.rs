use serde::{Deserialize, Serialize};

use crate::runfile::random_config::RandomConfig;

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct RandomGuessConfig {
    #[serde(flatten)]
    pub(crate) random: RandomConfig,
}
