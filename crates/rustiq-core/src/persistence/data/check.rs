use super::{
    hf_artifact::HF_REPRESENTATION, AoEriArtifact, Artifact, ConvergedHfArtifact, RustiQBundle,
};
use crate::persistence::{ArtifactError, PortableError, COMPACT_ERI_REPRESENTATION};
use std::path::Path;

/// The outcome of checking a single declared artifact.
#[derive(Debug)]
#[non_exhaustive]
pub enum ArtifactCheckStatus {
    /// The supported payload passed verification.
    Valid,
    /// Checking a supported payload returned an error.
    Invalid(ArtifactError),
    /// The artifact name is unrecognized. Its payload was skipped.
    UnknownArtifact,
    /// The name is recognized but the representation is unsupported. Its payload was skipped.
    UnsupportedRepresentation,
}

/// A check result for one logical artifact at a zero-based calculation index.
#[derive(Debug)]
#[non_exhaustive]
pub struct ArtifactCheck {
    pub calculation_index: usize,
    pub name: String,
    pub representation: String,
    pub status: ArtifactCheckStatus,
}

/// Results for every declared artifact, in calculation and artifact-name order.
#[derive(Debug)]
#[non_exhaustive]
pub struct ArtifactCheckSummary {
    pub calculation_count: usize,
    pub artifacts: Vec<ArtifactCheck>,
}

impl RustiQBundle {
    /// Opens a portable archive and checks each declared artifact.
    /// Missing members and size mismatches for supported artifacts are reported
    /// individually, so verification can continue with the remaining artifacts.
    ///
    /// # Errors
    /// Returns an error if the archive structure, manifest, snapshots, or provenance are invalid.
    pub fn check_archive(path: impl AsRef<Path>) -> Result<ArtifactCheckSummary, PortableError> {
        Ok(Self::open_inner(path.as_ref(), false)?.check())
    }

    /// Checks all supported numerical payloads without materializing their arrays.
    /// Unknown names and unsupported representations are reported without reading their payloads.
    #[must_use]
    pub fn check(&self) -> ArtifactCheckSummary {
        let mut artifacts = Vec::new();
        for (calculation_index, data) in self.calculations().iter().enumerate() {
            for (name, representation) in data.artifact_representations() {
                use ArtifactCheckStatus::{UnknownArtifact, UnsupportedRepresentation};
                let status = match (name, representation) {
                    (AoEriArtifact::NAME, COMPACT_ERI_REPRESENTATION) => {
                        checked(data.check::<AoEriArtifact>())
                    }
                    (ConvergedHfArtifact::NAME, HF_REPRESENTATION) => {
                        checked(data.check::<ConvergedHfArtifact>())
                    }
                    (AoEriArtifact::NAME | ConvergedHfArtifact::NAME, _) => {
                        UnsupportedRepresentation
                    }
                    _ => UnknownArtifact,
                };
                artifacts.push(ArtifactCheck {
                    calculation_index,
                    name: name.to_owned(),
                    representation: representation.to_owned(),
                    status,
                });
            }
        }
        ArtifactCheckSummary {
            calculation_count: self.calculations().len(),
            artifacts,
        }
    }
}

fn checked(result: Result<bool, ArtifactError>) -> ArtifactCheckStatus {
    match result {
        Ok(true) => ArtifactCheckStatus::Valid,
        Ok(false) => ArtifactCheckStatus::Invalid(ArtifactError::Missing),
        Err(error) => ArtifactCheckStatus::Invalid(error),
    }
}
