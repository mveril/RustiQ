pub(crate) mod distribution_config;
pub(crate) use distribution_config::DistributionConfig;
use rand::rngs::StdRng;
use rand::SeedableRng;
use toml_spanner::{helper::flatten_any, Toml};

use crate::runfile::random_config::distribution_config::SelectedSampleIter;

#[derive(Debug, Clone, Copy, Toml)]
#[toml(Toml)]
pub(crate) struct RandomConfig {
    #[toml(flatten, with = flatten_any)]
    pub(crate) distribution: DistributionConfig,
    #[toml(default)]
    pub(crate) seed: Option<u64>,
}

impl RandomConfig {
    pub fn sample_iter(
        &self,
    ) -> Result<SelectedSampleIter, distribution_config::DistributionCreationError> {
        let rng = if let Some(seed) = self.seed {
            StdRng::seed_from_u64(seed)
        } else {
            StdRng::from_rng(&mut rand::rng())
        };
        self.distribution.sample_iter(rng)
    }
}
