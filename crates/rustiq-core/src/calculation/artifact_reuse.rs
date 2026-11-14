use crate::persistence::{Artifact, ArtifactError, RustiQData};

use super::{ArtifactReuseDecision, ArtifactReuseEvent, PreparedCalculation};

/// The origin of reusable state does not change the scientific execution API.
pub(super) enum CalculationSource {
    Configuration,
    Portable(Box<ArtifactReuse>),
}

/// Read-only persistence input. Execution never stages implicit artifacts.
pub(super) struct ArtifactReuse {
    source: RustiQData,
}

impl ArtifactReuse {
    pub(super) fn new(source: RustiQData) -> Self {
        Self { source }
    }

    pub(super) fn resolve<A: Artifact>(
        &self,
        calculation: &PreparedCalculation,
    ) -> Result<(Option<A::Value>, ArtifactReuseEvent), ArtifactError> {
        let (value, decision) = if !A::is_present(&self.source) {
            (None, ArtifactReuseDecision::Missing)
        } else if !A::is_compatible(&self.source, calculation) {
            (None, ArtifactReuseDecision::Incompatible)
        } else {
            let value = self.source.get::<A>()?;
            let decision = if value.is_some() {
                ArtifactReuseDecision::Reused
            } else {
                ArtifactReuseDecision::Missing
            };
            (value, decision)
        };
        Ok((
            value,
            ArtifactReuseEvent {
                artifact: A::NAME,
                decision,
            },
        ))
    }
}
