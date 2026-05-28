use rand_distr::{Normal, NormalError};
use toml_spanner::Toml;

use crate::runfile::validated::PositiveFiniteF64;

#[derive(Debug, Clone, Copy, Toml)]
#[toml(Toml)]
pub(crate) struct NormalDistributionConfig {
    pub(crate) mean: f64,
    pub(crate) std_dev: PositiveFiniteF64,
}

impl TryFrom<NormalDistributionConfig> for Normal<f64> {
    type Error = NormalError;

    fn try_from(value: NormalDistributionConfig) -> Result<Self, Self::Error> {
        Normal::new(value.mean, value.std_dev.into_inner())
    }
}
