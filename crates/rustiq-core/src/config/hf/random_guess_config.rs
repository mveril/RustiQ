use crate::config::random_config::distribution_config::UniformDistributionConfig;
use crate::config::random_config::{DistributionConfig, RandomConfig};

#[derive(Debug, Clone, Copy)]
pub struct RandomGuessConfig {
    pub random: RandomConfig,
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
