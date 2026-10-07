mod normal_distribution_config;
mod uniform_distribution_config;
pub(crate) use normal_distribution_config::NormalDistributionConfig;
pub(crate) use uniform_distribution_config::UniformDistributionConfig;

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(tag = "distribution")]
pub(crate) enum DistributionConfig {
    Uniform {
        #[serde(flatten)]
        config: UniformDistributionConfig,
    },
    Normal {
        #[serde(flatten)]
        config: NormalDistributionConfig,
    },
}
#[cfg(test)]
mod tests {
    fn parse_distribution(source: &str) -> miette::Result<crate::runfile::RunFile> {
        crate::runfile::parse_section("method.hf.guess", &format!("type = 'Random'\n{source}"))
    }

    #[test]
    fn test_normal_distribution_rejects_non_positive_std_dev() {
        let zero = parse_distribution(
            r#"
            distribution = "Normal"
            mean = 0.0
            std_dev = 0.0
            "#,
        );
        let negative = parse_distribution(
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
        let equal = parse_distribution(
            r#"
            distribution = "Uniform"
            min = 1.0
            max = 1.0
            "#,
        );
        let reversed = parse_distribution(
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
