use crate::runfile::validated::PositiveFiniteF64;

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub(crate) struct NormalDistributionConfig {
    pub(crate) mean: f64,
    #[serde(serialize_with = "crate::runfile::validated::positive_finite_f64::serialize")]
    pub(crate) std_dev: PositiveFiniteF64,
}
