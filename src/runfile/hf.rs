use serde::{Deserialize, Serialize};

use crate::hf::density_guess::GuessType as DensityGuessType;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct HfConfig {
    #[serde(default = "default_max_iter")]
    pub max_iterations: usize,
    #[serde(default = "default_conv_threshold")]
    pub convergence_threshold: f64,
    #[serde(default)]
    pub density_guess: DensityGuessType,
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
}
