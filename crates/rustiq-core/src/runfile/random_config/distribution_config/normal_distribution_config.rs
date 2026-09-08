use toml_spanner::Toml;

use crate::runfile::validated::PositiveFiniteF64;

#[derive(Debug, Clone, Copy, Toml)]
#[toml(Toml)]
pub(crate) struct NormalDistributionConfig {
    pub(crate) mean: f64,
    pub(crate) std_dev: PositiveFiniteF64,
}
