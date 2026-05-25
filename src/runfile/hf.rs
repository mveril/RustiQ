use serde::{Deserialize, Serialize};

mod density_guess_config;
mod random_guess_config;

pub(crate) use density_guess_config::DensityGuessConfig;
pub(crate) use random_guess_config::RandomGuessConfig;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct HfConfig {
    #[serde(default = "default_max_iter")]
    pub max_iterations: usize,
    #[serde(default = "default_conv_threshold")]
    pub convergence_threshold: f64,
    #[serde(default)]
    pub guess: DensityGuessConfig,
    #[serde(default)]
    pub diis: bool,
    #[serde(default = "default_diis_size")]
    pub diis_size: usize,
    #[serde(default)]
    pub format: HfOutputFormat,
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) enum HfOutputFormat {
    #[default]
    Normal,
    Nope,
}

fn default_conv_threshold() -> f64 {
    1e-8
}

fn default_max_iter() -> usize {
    100
}

fn default_diis_size() -> usize {
    6
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::discriminant;

    #[test]
    fn test_hf_config_diis_defaults() {
        let config: HfConfig = toml::from_str("").unwrap();

        assert!(!config.diis);
        assert_eq!(config.diis_size, 6);
        assert_eq!(config.format, HfOutputFormat::Normal);
    }

    #[test]
    fn test_hf_config_diis_deserialization() {
        let config: HfConfig = toml::from_str(
            r#"
            diis = true
            diis_size = 8
            "#,
        )
        .unwrap();

        assert!(config.diis);
        assert_eq!(config.diis_size, 8);
    }

    #[test]
    fn test_hf_config_format_deserialization() {
        let normal: HfConfig = toml::from_str(r#"format = "Normal""#).unwrap();
        let nope: HfConfig = toml::from_str(r#"format = "Nope""#).unwrap();

        assert_eq!(normal.format, HfOutputFormat::Normal);
        assert_eq!(nope.format, HfOutputFormat::Nope);
    }

    #[test]
    fn test_hf_config_random_guess_deserialization() {
        let config: HfConfig = toml::from_str(
            r#"
            [guess]
            type = "Random"
            distribution = "Normal"
            mean = 0.0
            std_dev = 0.5
            seed = 42
            "#,
        )
        .unwrap();

        assert_eq!(
            discriminant(&config.guess),
            discriminant(&DensityGuessConfig::Random(RandomGuessConfig::default()))
        );
        let DensityGuessConfig::Random(guess_config) = config.guess else {
            panic!("expected random guess config");
        };
        assert_eq!(guess_config.random.seed, Some(42));
    }

    #[test]
    fn test_hf_config_serializes_random_config_only_for_random_guess() {
        let core_config: HfConfig = toml::from_str(
            r#"
            [guess]
            type = "CoreHamiltonian"
            "#,
        )
        .unwrap();
        let core_toml = toml::to_string(&core_config).unwrap();

        assert!(core_toml.contains("type = \"CoreHamiltonian\""));
        assert!(!core_toml.contains("distribution"));
        assert!(!core_toml.contains("min"));
        assert!(!core_toml.contains("max ="));

        let random_config: HfConfig = toml::from_str(
            r#"
            [guess]
            type = "RandomSymmetric"
            distribution = "Normal"
            mean = 0.0
            std_dev = 0.5
            seed = 42
            "#,
        )
        .unwrap();
        let random_toml = toml::to_string(&random_config).unwrap();

        assert!(random_toml.contains("type = \"RandomSymmetric\""));
        assert!(random_toml.contains("distribution = \"Normal\""));
        assert!(random_toml.contains("seed = 42"));
    }
}
