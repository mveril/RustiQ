use serde::{Deserialize, Serialize};

pub use crate::config::MemoryLimit;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Mp2Config {
    pub frozen_orbitals: usize,
    pub memory_limit: MemoryLimit,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytesize::ByteSize;

    fn parse_mp2(source: &str) -> miette::Result<Mp2Config> {
        super::super::parse_section("method.mp2", source).map(|runfile| runfile.method.mp2.unwrap())
    }

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
            let parsed: Mp2Config = parse_mp2(&source).unwrap();
            assert_eq!(parsed.memory_limit, value);
            let serialized = crate::runfile::output::to_string(&parsed).unwrap();
            let restored: Mp2Config = parse_mp2(&serialized).unwrap();
            assert_eq!(restored.memory_limit, value);
        }
        let config: Mp2Config = parse_mp2("").unwrap();
        assert_eq!(config.memory_limit, MemoryLimit::default());
        for text in ["auto", "AUTO", " Auto "] {
            assert_eq!(MemoryLimit::parse(text).unwrap(), MemoryLimit::Auto);
        }
        assert_eq!(MemoryLimit::Auto.exact(), "auto");
        let serialized = crate::runfile::output::to_string(&config).unwrap();
        assert_eq!(
            parse_mp2(&serialized).unwrap().memory_limit,
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
            assert!(parse_mp2(&source).is_err(), "{text}");
        }
        assert!(parse_mp2("memory_limit = 512").is_err());
    }

    #[test]
    fn test_mp2_config_defaults_to_no_frozen_orbitals() {
        let config: Mp2Config = parse_mp2("").unwrap();

        assert_eq!(config.frozen_orbitals, 0);
    }

    #[test]
    fn test_mp2_config_can_set_frozen_orbitals() {
        let config: Mp2Config = parse_mp2("frozen_orbitals = 1").unwrap();

        assert_eq!(config.frozen_orbitals, 1);
    }
}
