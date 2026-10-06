//! MP2 memory limits shared by configuration frontends and core adapters.
use bytesize::ByteSize;
use serde::{Deserialize, Serialize};

/// Memory limit selected for an MP2 calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MemoryLimit {
    /// Choose a limit from available system memory.
    #[default]
    Auto,
    /// An exact fixed limit in bytes.
    Fixed(ByteSize),
}

impl MemoryLimit {
    pub(crate) fn parse(text: &str) -> Result<Self, String> {
        let text = text.trim();
        if text.eq_ignore_ascii_case("auto") {
            return Ok(Self::Auto);
        }
        if !text.chars().any(|c| c.is_ascii_alphabetic()) {
            return Err("MP2 memory limit requires a unit, for example 512 MiB".into());
        }
        let size = text.parse::<ByteSize>().map_err(|error| error.clone())?;
        if size.as_u64() == 0 || size.as_u64() > isize::MAX as u64 {
            return Err("MP2 memory limit must be positive and fit the addressable range".into());
        }
        Ok(Self::Fixed(size))
    }

    /// Format a limit without losing any fixed bytes.
    pub(crate) fn exact(self) -> String {
        let Self::Fixed(size) = self else {
            return "auto".into();
        };
        let bytes = size.as_u64();
        for (unit, scale) in [("GiB", 1u64 << 30), ("MiB", 1 << 20), ("KiB", 1 << 10)] {
            if bytes % scale == 0 {
                return format!("{} {unit}", bytes / scale);
            }
        }
        format!("{bytes} B")
    }
}

impl Serialize for MemoryLimit {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.exact())
    }
}

impl<'de> Deserialize<'de> for MemoryLimit {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}
