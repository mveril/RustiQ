use toml_spanner::{helper::flatten_any, Toml};

use crate::runfile::random_config::distribution_config::UniformDistributionConfig;
use crate::runfile::random_config::{DistributionConfig, RandomConfig};

#[derive(Debug, Clone, Copy, Toml)]
#[toml(Toml)]
pub(crate) struct RandomGuessConfig {
    #[toml(flatten, with = flatten_any)]
    pub(crate) random: RandomConfig,
}

impl Default for RandomGuessConfig {
    fn default() -> Self {
        Self {
            random: RandomConfig {
                distribution: DistributionConfig::Uniform {
                    config: UniformDistributionConfig {
                        min: -1f64,
                        max: 1f64,
                    },
                },
                seed: None,
            },
        }
    }
}
