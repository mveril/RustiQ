pub(crate) mod distribution_config;
pub(crate) use distribution_config::DistributionConfig;
use toml_spanner::{helper::flatten_any, Toml};

#[derive(Debug, Clone, Copy, Toml)]
#[toml(Toml)]
pub(crate) struct RandomConfig {
    #[toml(flatten, with = flatten_any)]
    pub(crate) distribution: DistributionConfig,
    #[toml(default)]
    pub(crate) seed: Option<u64>,
}
