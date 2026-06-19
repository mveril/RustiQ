use serde::{Deserialize, Serialize};
use toml_spanner::Toml;

#[derive(Debug, Default, Serialize, Deserialize, Toml)]
#[toml(Toml)]
pub(crate) struct Mp2Config {
    #[toml(default)]
    #[toml(with = crate::runfile::validated::usize_as_integer)]
    pub frozen_orbitals: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mp2_config_defaults_to_no_frozen_orbitals() {
        let config: Mp2Config = toml_spanner::from_str("").unwrap();

        assert_eq!(config.frozen_orbitals, 0);
    }

    #[test]
    fn test_mp2_config_can_set_frozen_orbitals() {
        let config: Mp2Config = toml_spanner::from_str("frozen_orbitals = 1").unwrap();

        assert_eq!(config.frozen_orbitals, 1);
    }
}
