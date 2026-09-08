mod normal_distribution_config;
mod uniform_distribution_config;
pub(crate) use normal_distribution_config::NormalDistributionConfig;
pub(crate) use uniform_distribution_config::UniformDistributionConfig;

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
