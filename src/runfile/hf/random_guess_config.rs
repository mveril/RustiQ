use serde::{Deserialize, Serialize};

use crate::runfile::random_config::distribution_config::UniformDistributionConfig;
use crate::runfile::random_config::{DistributionConfig, RandomConfig};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct RandomGuessConfig {
    #[serde(flatten)]
    pub(crate) random: RandomConfig,
}

impl Default for RandomGuessConfig {
    fn default() -> Self {
        Self {
            random: RandomConfig {
                distribution: DistributionConfig::Uniform(UniformDistributionConfig {
                    min: -1f64,
                    max: 1f64,
                }),
                seed: None,
            },
        }
    }
}
