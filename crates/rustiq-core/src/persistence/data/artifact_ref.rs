use relative_path::RelativePath;

use super::{Artifact, RustiQData};
use crate::{
    eri::CompactEri,
    persistence::{
        write_compact_eri, AoEriAttributes, ArtifactAttributes, ArtifactError, ArtifactManifest,
        Storage, AO_ERI_COMPUTATION_VERSION, AO_ERI_PATH, COMPACT_ERI_REPRESENTATION,
    },
};

/// A typed, borrowed artifact to stream directly during one publication.
/// Construct one reference per artifact name with [`Self::new`].
#[derive(Clone, Copy, Debug)]
pub struct ArtifactRef<'a>(Value<'a>);

#[derive(Clone, Copy, Debug)]
enum Value<'a> {
    AoEri(&'a CompactEri),
}

impl<'a> ArtifactRef<'a> {
    /// Borrows a known scientific value without cloning or staging it.
    pub fn new<A: Artifact>(value: &'a A::Value) -> Self {
        A::borrow(value)
    }

    pub(super) fn ao_eri(value: &'a CompactEri) -> Self {
        Self(Value::AoEri(value))
    }

    pub(super) fn name(self) -> &'static str {
        match self.0 {
            Value::AoEri(_) => super::AO_ERI_ARTIFACT,
        }
    }

    pub(super) fn validate(self, data: &RustiQData) -> Result<(), ArtifactError> {
        match self.0 {
            Value::AoEri(eri) => data.validate_eri(eri),
        }
    }

    pub(super) fn write_to(
        self,
        data: &RustiQData,
        destination: &mut Storage,
    ) -> Result<ArtifactManifest, ArtifactError> {
        self.validate(data)?;
        match self.0 {
            Value::AoEri(eri) => {
                let metadata = destination.write_artifact::<ArtifactError, _>(
                    RelativePath::new(AO_ERI_PATH),
                    |writer| write_compact_eri(writer, eri).map_err(Into::into),
                )?;
                Ok(ArtifactManifest {
                    path: AO_ERI_PATH.into(),
                    size: metadata.size,
                    digest: metadata.digest,
                    representation: COMPACT_ERI_REPRESENTATION.into(),
                    attributes: ArtifactAttributes::AoEri(AoEriAttributes {
                        basis_functions: data.basis_functions.ok_or(ArtifactError::Missing)?,
                        computation_version: AO_ERI_COMPUTATION_VERSION,
                    }),
                })
            }
        }
    }
}
