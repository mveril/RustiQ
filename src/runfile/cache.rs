#[derive(Debug, Default, serde::Serialize)]
pub struct CacheConfig {
    pub enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse_cache(source: &str) -> miette::Result<CacheConfig> {
        super::super::parse_section("cache", source).map(|runfile| runfile.cache)
    }

    #[test]
    fn cache_is_disabled_by_default() {
        let config: CacheConfig = parse_cache("").unwrap();
        assert!(!config.enabled);
    }

    #[test]
    fn cache_can_be_enabled_explicitly() {
        let config: CacheConfig = parse_cache("enabled = true").unwrap();
        assert!(config.enabled);
    }
}
