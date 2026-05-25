mod distribution_config;
pub(crate) use distribution_config::DistributionConfig;
use rand::rngs::StdRng;
use rand::{Rng, RngExt, SeedableRng};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct RandomConfig {
    #[serde(flatten)]
    pub(crate) distribution: DistributionConfig,
    #[serde(default)]
    pub(crate) seed: Option<u64>,
}

impl RandomConfig {
    pub(crate) fn rng(&self) -> StdRng {
        let seed = self.seed.unwrap_or_else(|| rand::rng().random());
        StdRng::seed_from_u64(seed)
    }

    pub(crate) fn sample<R>(&self, rng: &mut R) -> f64
    where
        R: Rng + ?Sized,
    {
        self.distribution.sample(rng)
    }

    pub(crate) fn sample_iter(&self) -> impl Iterator<Item = f64> {
        self.distribution.sample_iter(self.rng())
    }
}
