use toml_spanner::{helper::flatten_any, Context, Failed, FromToml, Item, Toml};

use crate::runfile::random_config::distribution_config::NormalDistributionConfig;
use crate::runfile::random_config::{DistributionConfig, RandomConfig};
use crate::runfile::validated::PositiveFiniteF64;

#[derive(Debug, Clone, Copy, Toml)]
#[toml(ToToml)]
pub(crate) struct GuessPerturbationConfig {
    #[toml(flatten, with = flatten_any)]
    pub(crate) random: RandomConfig,
}

impl Default for GuessPerturbationConfig {
    fn default() -> Self {
        Self {
            random: default_random_config(),
        }
    }
}

impl<'de> FromToml<'de> for GuessPerturbationConfig {
    fn from_toml(ctx: &mut Context<'de>, item: &Item<'de>) -> Result<Self, Failed> {
        if item["distribution"].item().is_some() {
            let random = RandomConfig::from_toml(ctx, item)?;
            return Ok(Self { random });
        }

        let mut table = item.table_helper(ctx)?;
        let seed = table.optional("seed");
        table.require_empty()?;
        Ok(Self {
            random: RandomConfig {
                seed,
                ..default_random_config()
            },
        })
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
