//! Scientific configuration independent of source formats and environment defaults.
pub mod hf;
mod molecule;
pub mod random_config;
pub mod validated;
pub use molecule::MoleculeConfig;

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
    DensityGuessConfig, GuessPerturbationConfig, HfConfig, HfMethod, HfMethodResolutionError,
    RandomGuessConfig, ResolvedHfMethod,
};

/// MP2 options shared by restricted and unrestricted calculations.
#[derive(Debug, Clone, Copy, Default)]
pub struct Mp2Config {
    pub frozen_orbitals: Located<usize>,
}
