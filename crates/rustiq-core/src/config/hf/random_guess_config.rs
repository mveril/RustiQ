use crate::config::random_config::{DistributionConfig, RandomConfig};

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct RandomGuessConfig {
    pub random: RandomConfig,
}

impl Default for RandomGuessConfig {
    fn default() -> Self {
        Self {
            random: RandomConfig::new(DistributionConfig::Uniform {
                config:
                    crate::config::random_config::distribution_config::UniformDistributionConfig {
                        min: -1.0,
                        max: 1.0,
                    },
            }),
        }
    }
}
