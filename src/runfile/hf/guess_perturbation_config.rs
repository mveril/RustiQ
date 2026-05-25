use serde::{Deserialize, Deserializer, Serialize};

use crate::runfile::random_config::distribution_config::NormalDistributionConfig;
use crate::runfile::random_config::{DistributionConfig, RandomConfig};

#[derive(Debug, Clone, Copy, Serialize)]
pub(crate) struct GuessPerturbationConfig {
    #[serde(flatten)]
    pub(crate) random: RandomConfig,
}

impl Default for GuessPerturbationConfig {
    fn default() -> Self {
        Self {
            random: RandomConfig {
                distribution: DistributionConfig::Normal(NormalDistributionConfig {
                    mean: 0.0,
                    std_dev: 1e-4,
                }),
                seed: None,
            },
        }
    }
}

impl<'de> Deserialize<'de> for GuessPerturbationConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawGuessPerturbationConfig {
            #[serde(flatten)]
            distribution: Option<DistributionConfig>,
            #[serde(default)]
            seed: Option<u64>,
        }

        let raw = RawGuessPerturbationConfig::deserialize(deserializer)?;
        let default = Self::default();
        Ok(Self {
            random: RandomConfig {
                distribution: raw.distribution.unwrap_or(default.random.distribution),
                seed: raw.seed,
            },
        })
    }
}
