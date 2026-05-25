use rand_distr::{Normal, NormalError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct NormalDistributionConfig {
    pub(crate) mean: f64,
    pub(crate) std_dev: f64,
}

impl TryFrom<NormalDistributionConfig> for Normal<f64> {
    type Error = NormalError;

    fn try_from(value: NormalDistributionConfig) -> Result<Self, Self::Error> {
        Normal::new(value.mean, value.std_dev)
    }
}
