use rand_distr::{Normal, NormalError};

use crate::config::validated::PositiveFiniteF64;

#[derive(Debug, Clone, Copy)]
pub struct NormalDistributionConfig {
    pub mean: f64,
    pub std_dev: PositiveFiniteF64,
}

impl TryFrom<NormalDistributionConfig> for Normal<f64> {
    type Error = NormalError;

    fn try_from(value: NormalDistributionConfig) -> Result<Self, Self::Error> {
        Normal::new(value.mean, value.std_dev.into_inner())
    }
}
