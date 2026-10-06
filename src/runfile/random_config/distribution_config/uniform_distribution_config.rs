#[derive(Debug, Clone, Copy, serde::Serialize)]
pub(crate) struct UniformDistributionConfig {
    pub min: f64,
    pub max: f64,
}
