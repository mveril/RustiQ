use std::fmt;

use thiserror::Error;

use super::AnyHfSolution;

/// The HF reference carried by a runtime solution.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HfReferenceKind {
    /// Restricted Hartree–Fock.
    Restricted,
    /// Unrestricted Hartree–Fock.
    Unrestricted,
}

impl fmt::Display for HfReferenceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Restricted => f.write_str("restricted"),
            Self::Unrestricted => f.write_str("unrestricted"),
        }
    }
}

/// Error returned when extracting a concrete HF reference from a runtime solution fails.
///
/// The original solution is retained so callers can recover it without cloning its scientific
/// data using [`into_solution`](Self::into_solution).
#[derive(Error)]
#[error("HF reference mismatch: expected {expected}, found {actual}")]
pub struct HfReferenceMismatch<State> {
    expected: HfReferenceKind,
    actual: HfReferenceKind,
    solution: AnyHfSolution<State>,
}

impl<State> HfReferenceMismatch<State> {
    pub(super) fn new(
        expected: HfReferenceKind,
        actual: HfReferenceKind,
        solution: AnyHfSolution<State>,
    ) -> Self {
        Self {
            expected,
            actual,
            solution,
        }
    }

    /// Returns the reference requested by the extraction.
    #[must_use]
    pub fn expected(&self) -> HfReferenceKind {
        self.expected
    }

    /// Returns the reference present in the original solution.
    #[must_use]
    pub fn actual(&self) -> HfReferenceKind {
        self.actual
    }

    /// Recovers the original runtime solution without cloning scientific data.
    #[must_use]
    pub fn into_solution(self) -> AnyHfSolution<State> {
        self.solution
    }
}

impl<State> fmt::Debug for HfReferenceMismatch<State> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HfReferenceMismatch")
            .field("expected", &self.expected)
            .field("actual", &self.actual)
            .finish_non_exhaustive()
    }
}
