//! Scientific configuration independent of source formats and environment defaults.
//!
//! Import configuration types, random distributions, and validated scalar values
//! directly from this module. [`Located`] attaches optional frontend source
//! positions without requiring a source file for direct Rust calculations.
//!
//! Extensible option structs are non-exhaustive. Start from [`Default`] and
//! change the public fields you need; future options must default to behavior
//! compatible with existing configurations. This applies to both core defaults
//! and the defaults supplied by an input frontend.
//!
//! ```
//! use rustiq_core::config::{HfConfig, HfMethod, Mp2Config};
//! let mut hf = HfConfig::default();
//! hf.method = HfMethod::Uhf.into();
//! let mut mp2 = Mp2Config::default();
//! mp2.frozen_orbitals = 1.into();
//! ```
//!
//! Struct literals, including update syntax, cannot construct these options
//! outside this crate:
//!
//! ```compile_fail
//! use rustiq_core::config::Mp2Config;
//! let mp2 = Mp2Config { frozen_orbitals: 1.into(), ..Default::default() };
//! ```
pub(crate) mod hf;
mod integrals;
mod molecule;
pub(crate) mod random_config;
pub(crate) mod validated;
use bytesize::ByteSize;
pub use molecule::{MoleculeConfig, MoleculeConfigError};
pub use random_config::{
    distribution_config::{NormalDistributionConfig, UniformDistributionConfig},
    DistributionConfig, DistributionCreationError, RandomConfig,
};
pub use validated::{
    DiisSize, DiisSizeError, NonNegativeFiniteF64, NonNegativeFiniteF64Error, PositiveFiniteF64,
    PositiveFiniteF64Error,
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
    DensityGuessConfig, DiisConfig, GuessPerturbationConfig, HfConfig, HfConfigError, HfMethod,
    HfMethodConversionError, HfMethodResolutionError, OrthogonalizationConfig, RandomGuessConfig,
    ResolvedHfMethod, RestrictedMethod, UnrestrictedMethod,
};
pub use integrals::{IntegralConfig, DEFAULT_ERI_SCHWARZ_THRESHOLD};

/// MP2 options shared by restricted and unrestricted calculations.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct Mp2Config {
    pub frozen_orbitals: Located<usize>,
    /// Additional MP2 matrix workspace budget; excludes existing HF data.
    pub memory_limit: Located<MemoryLimit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
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
    #[must_use]
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
