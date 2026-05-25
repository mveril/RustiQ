mod normal_distribution_config;
mod uniform_distribution_config;
pub(crate) use normal_distribution_config::NormalDistributionConfig;
pub(crate) use uniform_distribution_config::UniformDistributionConfig;

use rand::distr::Distribution as RandDistribution;
use rand::distr::Uniform;
use rand::{Rng, RngExt};
use rand_distr::Normal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "distribution")]
pub(crate) enum DistributionConfig {
    Uniform(UniformDistributionConfig),
    Normal(NormalDistributionConfig),
}

impl Default for DistributionConfig {
    fn default() -> Self {
        Self::Uniform(UniformDistributionConfig::default())
    }
}

impl DistributionConfig {
    pub(crate) fn sample<R>(&self, rng: &mut R) -> f64
    where
        R: Rng + ?Sized,
    {
        match *self {
            Self::Uniform(config) => {
                let distribution: Uniform<f64> = config
                    .try_into()
                    .expect("invalid uniform random distribution");
                RandDistribution::sample(&distribution, rng)
            }
            Self::Normal(config) => {
                let distribution: Normal<f64> = config
                    .try_into()
                    .expect("invalid normal random distribution");
                RandDistribution::sample(&distribution, rng)
            }
        }
    }

    pub(crate) fn sample_iter<R>(&self, rng: R) -> Box<dyn Iterator<Item = f64>>
    where
        R: Rng + 'static,
    {
        match *self {
            Self::Uniform(config) => {
                let distribution: Uniform<f64> = config
                    .try_into()
                    .expect("invalid uniform random distribution");
                Box::new(rng.sample_iter(distribution))
            }
            Self::Normal(config) => {
                let distribution: Normal<f64> = config
                    .try_into()
                    .expect("invalid normal random distribution");
                Box::new(rng.sample_iter(distribution))
            }
        }
    }
}
