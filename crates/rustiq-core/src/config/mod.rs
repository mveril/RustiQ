//! Scientific configuration independent of source formats and environment defaults.
pub mod hf;
mod molecule;
pub mod random_config;
pub mod validated;
use bytesize::ByteSize;
pub use molecule::{MoleculeConfig, MoleculeConfigError};
use sys_info::mem_info;

/// A value with an optional byte range in a source owned by its frontend.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Located<T> {
    pub value: T,
    pub span: Option<miette::SourceSpan>,
}

impl<T> From<T> for Located<T> {
    fn from(value: T) -> Self {
        Self { value, span: None }
    }
}

impl<T> Located<T> {
    /// Extract the value, discarding its optional source location.
    pub fn into_inner(self) -> T {
        self.value
    }
}
pub use hf::{
    DensityGuessConfig, GuessPerturbationConfig, HfConfig, HfConfigError, HfMethod,
    HfMethodResolutionError, RandomGuessConfig, ResolvedHfMethod,
};

/// MP2 options shared by restricted and unrestricted calculations.
#[derive(Debug, Clone, Copy)]
pub struct Mp2Config {
    pub frozen_orbitals: Located<usize>,
    /// Additional MP2 matrix workspace budget; excludes existing HF data.
    pub memory_limit: Located<MemoryLimit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MemoryLimit {
    #[default]
    Auto,
    Fixed(ByteSize),
}

impl MemoryLimit {
    /// Resolve once per MP2 calculation, before allocating transformation buffers.
    pub fn resolve(self) -> ByteSize {
        match self {
            Self::Auto => Self::automatic_memory_limit(),
            Self::Fixed(value) => value,
        }
    }

    fn automatic_memory_limit() -> ByteSize {
        Self::from_available_memory(mem_info().ok().map(|m| (m.avail, m.free)))
    }

    fn from_available_memory(memory_kib: Option<(u64, u64)>) -> ByteSize {
        // sys-info reports KiB. Available memory includes reclaimable caches;
        // some platforms only populate the free-memory field.
        let bytes = memory_kib
            .and_then(|(available, free)| {
                let available = if available == 0 { free } else { available };
                available
                    .checked_mul(1024)
                    .map(|bytes| bytes / 2)
                    .filter(|&bytes| bytes > 0)
            })
            .unwrap_or(512 * 1024 * 1024);
        ByteSize::b(bytes.min(isize::MAX as u64))
    }
}

impl Default for Mp2Config {
    fn default() -> Self {
        Self {
            frozen_orbitals: 0.into(),
            memory_limit: MemoryLimit::default().into(),
        }
    }
}

#[cfg(test)]
mod memory_tests {
    use super::*;

    #[test]
    fn automatic_memory_uses_available_kib_and_safe_fallback() {
        for (input, expected) in [
            (Some((4096, 1024)), 2 * 1024 * 1024),
            (Some((0, 4096)), 2 * 1024 * 1024),
            (None, 512 * 1024 * 1024),
            (Some((0, 0)), 512 * 1024 * 1024),
            (Some((u64::MAX, 1)), 512 * 1024 * 1024),
        ] {
            assert_eq!(MemoryLimit::from_available_memory(input).as_u64(), expected);
        }
        assert_eq!(MemoryLimit::Fixed(ByteSize::b(513)).resolve().as_u64(), 513);
        assert_eq!(Mp2Config::default().memory_limit.value, MemoryLimit::Auto);
    }
}
