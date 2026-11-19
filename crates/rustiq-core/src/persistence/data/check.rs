use super::{
    hf_artifact::HF_REPRESENTATION, AoEriArtifact, Artifact, ConvergedHfArtifact, RustiQBundle,
};
use crate::persistence::{ArtifactError, COMPACT_ERI_REPRESENTATION};

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
