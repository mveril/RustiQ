use serde::{Deserialize, Serialize};
use toml_spanner::Toml;

pub use crate::config::MemoryLimit;

#[derive(Debug, Default, Serialize, Deserialize, Toml)]
#[toml(Toml)]
pub struct Mp2Config {
    #[toml(default)]
    #[toml(with = crate::runfile::validated::usize_as_integer)]
    pub frozen_orbitals: usize,
    #[toml(default)]
    pub memory_limit: MemoryLimit,
}

impl<'de> toml_spanner::FromToml<'de> for MemoryLimit {
    fn from_toml(
        ctx: &mut toml_spanner::Context<'de>,
        item: &toml_spanner::Item<'de>,
    ) -> Result<Self, toml_spanner::Failed> {
        let text = <String as toml_spanner::FromToml>::from_toml(ctx, item)?;
        Self::parse(&text).map_err(|error| ctx.report_custom_error(error, item))
    }
}

impl toml_spanner::ToToml for MemoryLimit {
    fn to_toml<'a>(
        &'a self,
        arena: &'a toml_spanner::Arena,
    ) -> Result<toml_spanner::Item<'a>, toml_spanner::ToTomlError> {
        Ok(toml_spanner::Item::from(arena.alloc_str(&self.exact())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytesize::ByteSize;

    #[test]
    fn memory_sizes_parse_and_round_trip_exactly() {
        for (text, bytes) in [
            ("512 MiB", 512 * 1024 * 1024),
            ("1 GiB", 1 << 30),
            ("1.5 GiB", 3 << 29),
            ("500 MB", 500_000_000),
            (" 2 KiB ", 2048),
            ("9007199254740993 B", 9_007_199_254_740_993),
            ("513 B", 513),
        ] {
            let value = MemoryLimit::parse(text).unwrap();
            assert_eq!(value, MemoryLimit::Fixed(ByteSize::b(bytes)));
            assert_eq!(MemoryLimit::parse(&value.exact()).unwrap(), value);
            let json = serde_json::to_string(&value).unwrap();
            assert_eq!(serde_json::from_str::<MemoryLimit>(&json).unwrap(), value);
            let source = format!("memory_limit = {text:?}");
            let parsed: Mp2Config = toml_spanner::from_str(&source).unwrap();
            assert_eq!(parsed.memory_limit, value);
            let serialized = toml_spanner::to_string(&parsed).unwrap();
            let restored: Mp2Config = toml_spanner::from_str(&serialized).unwrap();
            assert_eq!(restored.memory_limit, value);
        }
        let config: Mp2Config = toml_spanner::from_str("").unwrap();
        assert_eq!(config.memory_limit, MemoryLimit::default());
        for text in ["auto", "AUTO", " Auto "] {
            assert_eq!(MemoryLimit::parse(text).unwrap(), MemoryLimit::Auto);
        }
        assert_eq!(MemoryLimit::Auto.exact(), "auto");
        let serialized = toml_spanner::to_string(&config).unwrap();
        assert_eq!(
            toml_spanner::from_str::<Mp2Config>(&serialized)
                .unwrap()
                .memory_limit,
            MemoryLimit::Auto
        );
        let json = serde_json::to_string(&MemoryLimit::Auto).unwrap();
        assert_eq!(
            serde_json::from_str::<MemoryLimit>(&json).unwrap(),
            MemoryLimit::Auto
        );
    }

    #[test]
    fn invalid_memory_sizes_are_rejected() {
        for text in [
            "0 B",
            "-1 GiB",
            "NaN GiB",
            "inf GiB",
            "512",
            "1 potato",
            "18446744073709551616 GiB",
            "999999999999999999999999999999999999999999 B",
        ] {
            assert!(MemoryLimit::parse(text).is_err(), "{text}");
            let source = format!("memory_limit = {text:?}");
            assert!(
                toml_spanner::from_str::<Mp2Config>(&source).is_err(),
                "{text}"
            );
        }
        assert!(toml_spanner::from_str::<Mp2Config>("memory_limit = 512").is_err());
    }

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
