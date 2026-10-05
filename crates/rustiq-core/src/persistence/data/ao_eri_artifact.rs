use crate::{eri::CompactEri, persistence::ArtifactError};

use super::{
    artifact::{private, Artifact},
    rustiq_data::{RustiQData, AO_ERI_ARTIFACT},
};

/// AO electron-repulsion integrals in compact storage order.
pub struct AoEriArtifact;

impl private::Sealed for AoEriArtifact {}

impl Artifact for AoEriArtifact {
    type Value = CompactEri;

    const NAME: &'static str = AO_ERI_ARTIFACT;

    fn is_present(data: &RustiQData) -> bool {
        data.ao_eri.is_some() || data.manifest.artifacts.contains_key(AO_ERI_ARTIFACT)
    }

    fn is_compatible(
        data: &RustiQData,
        calculation: &crate::calculation::PreparedCalculation,
    ) -> bool {
        data.matches_eri_identity(calculation)
    }

    fn get(data: &mut RustiQData) -> Result<Option<&CompactEri>, ArtifactError> {
        if data.ao_eri.is_some() {
            return Ok(data.ao_eri.as_ref());
        }
        if !data.manifest.artifacts.contains_key(AO_ERI_ARTIFACT) {
            return Ok(None);
        }
        data.read_eri().map(Some)
    }

    fn set(data: &mut RustiQData, value: CompactEri) -> Result<(), ArtifactError> {
        data.set_eri(value)
    }
}
