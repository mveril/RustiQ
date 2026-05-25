pub(crate) mod distribution_config;
pub(crate) use distribution_config::DistributionConfig;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct RandomConfig {
    #[serde(flatten)]
    pub(crate) distribution: DistributionConfig,
    #[serde(default)]
    pub(crate) seed: Option<u64>,
}

impl RandomConfig {
    pub(crate) fn sample_iter(&self) -> Box<dyn Iterator<Item = f64>> {
        let rng = if let Some(seed) = self.seed {
            StdRng::seed_from_u64(seed)
        } else {
            StdRng::from_rng(&mut rand::rng())
        };
        self.distribution.sample_iter(rng)
    }
}
