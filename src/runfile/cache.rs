use toml_spanner::Toml;

#[derive(Debug, Default, Toml)]
#[toml(Toml, recoverable)]
pub struct CacheConfig {
    #[toml(default)]
    pub enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_is_disabled_by_default() {
        let config: CacheConfig = toml_spanner::from_str("").unwrap();
        assert!(!config.enabled);
    }

    #[test]
    fn cache_can_be_enabled_explicitly() {
        let config: CacheConfig = toml_spanner::from_str("enabled = true").unwrap();
        assert!(config.enabled);
    }
}
