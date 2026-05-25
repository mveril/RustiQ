use rand::distr::uniform::{Error, Uniform};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct UniformDistributionConfig {
    pub min: f64,
    pub max: f64,
}

impl TryFrom<UniformDistributionConfig> for Uniform<f64> {
    type Error = Error;

    fn try_from(value: UniformDistributionConfig) -> Result<Self, Self::Error> {
        Uniform::new(value.min, value.max)
    }
}
