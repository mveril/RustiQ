pub(crate) mod distribution_config;
pub(crate) use distribution_config::DistributionConfig;

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub(crate) struct RandomConfig {
    #[serde(flatten)]
    pub(crate) distribution: DistributionConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) seed: Option<u64>,
}
