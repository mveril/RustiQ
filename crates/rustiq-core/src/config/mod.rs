//! Scientific configuration independent of source formats and environment defaults.
pub mod hf;
mod molecule;
pub mod random_config;
pub mod validated;
use bytesize::ByteSize;
pub use molecule::{MoleculeConfig, MoleculeConfigError};

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
    HfMethodResolutionError, RandomGuessConfig, ResolvedHfMethod, DEFAULT_ERI_SCHWARZ_THRESHOLD,
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
    ///
    /// Machine-dependent resource discovery is owned by the execution-resource
    /// layer rather than by scientific configuration.
    pub fn resolve(self) -> ByteSize {
        crate::resources::resolve_mp2_memory_limit(self)
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
    fn mp2_memory_defaults_to_auto() {
        assert_eq!(Mp2Config::default().memory_limit.value, MemoryLimit::Auto);
    }
}
