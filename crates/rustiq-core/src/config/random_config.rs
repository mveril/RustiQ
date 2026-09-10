pub use distribution_config::DistributionCreationError;
pub mod distribution_config;
pub use distribution_config::DistributionConfig;
use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::config::random_config::distribution_config::SelectedSampleIter;

#[derive(Debug, Clone, Copy)]
pub struct RandomConfig {
    pub distribution: DistributionConfig,
    pub seed: Option<u64>,
}

impl RandomConfig {
    pub(crate) fn sample_iter(
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
