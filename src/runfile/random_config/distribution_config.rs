mod normal_distribution_config;
mod uniform_distribution_config;
pub(crate) use normal_distribution_config::NormalDistributionConfig;
pub(crate) use uniform_distribution_config::UniformDistributionConfig;

use delegate::delegate;
use rand::distr::{
    uniform::{Error as UniformError, Uniform},
    Distribution,
};
use rand::rngs::StdRng;
use rand_distr::{Normal, NormalError};
use thiserror::Error;
use toml_spanner::{helper::flatten_any, Toml};

#[derive(Debug, Clone, Copy, Toml)]
#[toml(Toml, tag = "distribution")]
pub(crate) enum DistributionConfig {
    Uniform {
        #[toml(flatten, with = flatten_any)]
        config: UniformDistributionConfig,
    },
    Normal {
        #[toml(flatten, with = flatten_any)]
        config: NormalDistributionConfig,
    },
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
            Self::Uniform { config } => {
                let distribution: Uniform<f64> = config.try_into()?;
                Ok(RandomSampleIter::new(rng.into(), distribution).into())
            }
            Self::Normal { config } => {
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
    delegate! {
        to match self {
            SelectedSampleIter::UniformSampleIter(sampler) => sampler,
            SelectedSampleIter::NormalSampleIter(sampler) => sampler,
        } {
            fn sample(&mut self) -> f64;
        }
    }
}

impl Iterator for SelectedSampleIter {
    type Item = f64;

    delegate! {
        to match self {
            Self::UniformSampleIter(iter) => iter,
            Self::NormalSampleIter(iter) => iter,
        } {
            fn next(&mut self) -> Option<Self::Item>;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_distribution_rejects_non_positive_std_dev() {
        let zero = toml_spanner::from_str::<DistributionConfig>(
            r#"
            distribution = "Normal"
            mean = 0.0
            std_dev = 0.0
            "#,
        );
        let negative = toml_spanner::from_str::<DistributionConfig>(
            r#"
            distribution = "Normal"
            mean = 0.0
            std_dev = -0.1
            "#,
        );

        assert!(zero.is_err());
        assert!(negative.is_err());
    }

    #[test]
    fn test_uniform_distribution_rejects_invalid_range() {
        let equal = toml_spanner::from_str::<DistributionConfig>(
            r#"
            distribution = "Uniform"
            min = 1.0
            max = 1.0
            "#,
        );
        let reversed = toml_spanner::from_str::<DistributionConfig>(
            r#"
            distribution = "Uniform"
            min = 1.0
            max = -1.0
            "#,
        );

        assert!(equal.is_err());
        assert!(reversed.is_err());
    }
}
