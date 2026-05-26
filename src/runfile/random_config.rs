pub(crate) mod distribution_config;
pub(crate) use distribution_config::DistributionConfig;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};

use crate::runfile::random_config::distribution_config::SelectedSampleIter;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct RandomConfig {
    #[serde(flatten)]
    pub(crate) distribution: DistributionConfig,
    #[serde(default)]
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
        Ok(self.distribution.sample_iter(rng)?)
    }
}
