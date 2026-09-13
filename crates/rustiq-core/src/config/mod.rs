//! Scientific configuration independent of source formats and environment defaults.
pub mod hf;
mod molecule;
pub mod random_config;
pub mod validated;
use bytesize::ByteSize;
pub use molecule::{MoleculeConfig, MoleculeConfigError};
use sysinfo::{
    get_current_pid, MemoryRefreshKind, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System,
};

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
        Self::from_available_memory(Self::available_memory())
    }

    fn available_memory() -> Option<u64> {
        if !sysinfo::IS_SUPPORTED_SYSTEM {
            return None;
        }

        let mut system = System::new_with_specifics(
            RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram()),
        );
        let host_available = system.available_memory();

        let cgroup_available = get_current_pid().ok().and_then(|pid| {
            system.refresh_processes_specifics(
                ProcessesToUpdate::Some(&[pid]),
                false,
                ProcessRefreshKind::nothing(),
            );
            system
                .process(pid)
                .and_then(|process| process.cgroup_limits())
                .map(|limits| limits.free_memory)
        });

        Some(cgroup_available.map_or(host_available, |available| host_available.min(available)))
    }

    fn from_available_memory(available: Option<u64>) -> ByteSize {
        const FALLBACK: u64 = 512 * 1024 * 1024;

        let bytes = available.map(|bytes| bytes / 2).unwrap_or(FALLBACK);
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
    fn automatic_memory_uses_half_available_bytes_and_safe_fallback() {
        for (input, expected) in [
            (Some(4 * 1024 * 1024), 2 * 1024 * 1024),
            (Some(1), 0),
            (Some(0), 0),
            (None, 512 * 1024 * 1024),
            (Some(u64::MAX), isize::MAX as u64),
        ] {
            assert_eq!(MemoryLimit::from_available_memory(input).as_u64(), expected);
        }
        assert_eq!(MemoryLimit::Fixed(ByteSize::b(513)).resolve().as_u64(), 513);
        assert_eq!(Mp2Config::default().memory_limit.value, MemoryLimit::Auto);
    }
}
