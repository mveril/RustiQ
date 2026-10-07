use crate::runfile::random_config::distribution_config::NormalDistributionConfig;
use crate::runfile::random_config::{DistributionConfig, RandomConfig};
use crate::runfile::validated::PositiveFiniteF64;

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct GuessPerturbationConfig {
    #[serde(flatten)]
    pub(crate) random: RandomConfig,
}

impl Default for GuessPerturbationConfig {
    fn default() -> Self {
        Self {
            random: default_random_config(),
        }
    }
}

fn default_random_config() -> RandomConfig {
    RandomConfig {
        distribution: DistributionConfig::Normal {
            config: NormalDistributionConfig {
                mean: 0.0,
                std_dev: PositiveFiniteF64::try_new(1e-4)
                    .expect("default perturbation standard deviation is positive and finite"),
            },
        },
        seed: None,
    }
}
