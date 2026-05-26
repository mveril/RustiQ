mod normal_distribution_config;
mod uniform_distribution_config;
pub(crate) use normal_distribution_config::NormalDistributionConfig;
pub(crate) use uniform_distribution_config::UniformDistributionConfig;

use rand::distr::{
    uniform::{Error as UniformError, Uniform},
    Distribution,
};
use rand::rngs::StdRng;
use rand_distr::{Normal, NormalError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "distribution")]
pub(crate) enum DistributionConfig {
    Uniform(UniformDistributionConfig),
    Normal(NormalDistributionConfig),
}
#[derive(Debug, Error)]
#[error("Random distribution creation error: {0}")]
pub(crate) enum DistributionCreationError {
    #[error("Error on creation of uniform distribution {0}")]
    Uniform(#[from] UniformError),
    #[error("Error on creation of normal distribution {0}")]
    Normal(#[from] NormalError),
}

impl DistributionConfig {
    pub(crate) fn sample_iter<R>(
        &self,
        rng: R,
    ) -> Result<SelectedSampleIter, DistributionCreationError>
    where
        R: Into<StdRng>,
    {
        match *self {
            Self::Uniform(config) => {
                let distribution: Uniform<f64> = config.try_into()?;
                Ok(RandomSampleIter::new(rng.into(), distribution).into())
            }
            Self::Normal(config) => {
                let distribution: Normal<f64> = config.try_into()?;
                Ok(RandomSampleIter::new(rng.into(), distribution).into())
            }
        }
    }
}

pub(crate) trait RandomSampler {
    fn sample(&mut self) -> f64;
}

pub(crate) enum SelectedSampleIter {
    UniformSampleIter(RandomSampleIter<Uniform<f64>>),
    NormalSampleIter(RandomSampleIter<Normal<f64>>),
}

impl RandomSampler for SelectedSampleIter {
    fn sample(&mut self) -> f64 {
        match self {
            SelectedSampleIter::UniformSampleIter(sampler) => sampler.sample(),
            SelectedSampleIter::NormalSampleIter(sampler) => sampler.sample(),
        }
    }
}

impl Iterator for SelectedSampleIter {
    type Item = f64;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::UniformSampleIter(iter) => iter.next(),
            Self::NormalSampleIter(iter) => iter.next(),
        }
    }
}

impl<T: Distribution<f64>> Iterator for RandomSampleIter<T> {
    type Item = f64;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.sample())
    }
}

pub(crate) struct RandomSampleIter<T: Distribution<f64>> {
    rng: StdRng,
    distribution: T,
}

impl<T: Distribution<f64>> RandomSampleIter<T> {
    fn new(rng: StdRng, distribution: T) -> Self {
        Self { rng, distribution }
    }
}

impl<T: Distribution<f64>> RandomSampler for RandomSampleIter<T> {
    fn sample(&mut self) -> f64 {
        self.distribution.sample(&mut self.rng)
    }
}

impl From<RandomSampleIter<Uniform<f64>>> for SelectedSampleIter {
    fn from(value: RandomSampleIter<Uniform<f64>>) -> Self {
        SelectedSampleIter::UniformSampleIter(value)
    }
}

impl From<RandomSampleIter<Normal<f64>>> for SelectedSampleIter {
    fn from(value: RandomSampleIter<Normal<f64>>) -> Self {
        SelectedSampleIter::NormalSampleIter(value)
    }
}
