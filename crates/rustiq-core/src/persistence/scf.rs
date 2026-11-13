//! Method-specific persistence adapters for converged SCF output.
use crate::calculation::{Converged, ScfMethod, ScfSolution};

use super::{Artifact, ArtifactError};

/// Defines the versioned artifact produced by a converged SCF method.
///
/// Implementations own the scientific identity, representation and validation contract.
/// The associated artifact controls typed storage access; no common orbital layout or
/// retained integral representation is required.
/// Conversion is available only for converged solutions:
///
/// ```compile_fail
/// use rustiq_core::{calculation::{ScfSolution, Unconverged}, persistence::PersistScf};
/// fn save<M: PersistScf>(solution: &ScfSolution<M, Unconverged>) {
///     let _ = solution.to_artifact();
/// }
/// ```
pub trait PersistScf: ScfMethod + Sized {
    type Artifact: Artifact;

    /// Converts converged output into independently owned persistence data.
    /// Heavy retained execution data should not be copied or retained by the artifact.
    ///
    /// # Errors
    /// Returns an error if the method cannot represent the solution in its artifact format.
    fn to_artifact(
        solution: &ScfSolution<Self, Converged>,
    ) -> Result<<Self::Artifact as Artifact>::Value, ArtifactError>;
}

impl<M: PersistScf> ScfSolution<M, Converged> {
    /// Converts this converged solution using its method's persistence contract.
    ///
    /// # Errors
    /// Returns an error if the method-specific conversion fails.
    pub fn to_artifact(&self) -> Result<<M::Artifact as Artifact>::Value, ArtifactError> {
        M::to_artifact(self)
    }
}
