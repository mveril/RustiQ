use std::sync::Mutex;

use crate::persistence::{Artifact, ArtifactError, PortableError, RustiQData};

use super::{ArtifactReuseDecision, ArtifactReuseEvent, PreparedCalculation};

/// The origin of reusable state does not change the scientific execution API.
pub(super) enum CalculationSource {
    Configuration,
    Portable(Box<ArtifactReuse>),
}

/// Read-only persistence input with lazily decoded typed domain values.
/// Numerical algorithms receive domain objects, never archive paths or keys.
pub(super) struct ArtifactReuse {
    source: Mutex<RustiQData>,
    resolved: Mutex<RustiQData>,
}

impl ArtifactReuse {
    pub(super) fn new(
        data: RustiQData,
        calculation: &PreparedCalculation,
    ) -> Result<Self, PortableError> {
        Ok(Self {
            source: Mutex::new(data),
            resolved: Mutex::new(RustiQData::from_calculation(calculation)?),
        })
    }

    pub(super) fn resolve<A: Artifact>(
        &self,
        calculation: &PreparedCalculation,
    ) -> Result<(Option<A::Value>, ArtifactReuseEvent), ArtifactError>
    where
        A::Value: Clone,
    {
        let mut resolved = self.resolved.lock().map_err(|_| {
            ArtifactError::AccessFailed("resolved artifact mutex is poisoned".into())
        })?;
        if let Some(value) = resolved.get::<A>()?.cloned() {
            return Ok((
                Some(value),
                ArtifactReuseEvent {
                    artifact: A::NAME,
                    decision: ArtifactReuseDecision::Reused,
                },
            ));
        }
        let mut data = self
            .source
            .lock()
            .map_err(|_| ArtifactError::AccessFailed("artifact source mutex is poisoned".into()))?;
        let (value, decision) = if !A::is_present(&data) {
            (None, ArtifactReuseDecision::Missing)
        } else if !A::is_compatible(&data, calculation) {
            (None, ArtifactReuseDecision::Incompatible)
        } else {
            let value = data.get::<A>()?.cloned();
            let decision = if value.is_some() {
                ArtifactReuseDecision::Reused
            } else {
                ArtifactReuseDecision::Missing
            };
            (value, decision)
        };
        if let Some(value) = &value {
            resolved.set::<A>(value.clone())?;
        }
        Ok((
            value,
            ArtifactReuseEvent {
                artifact: A::NAME,
                decision,
            },
        ))
    }

    /// Retains newly computed domain values in memory; never writes the source archive.
    pub(super) fn remember<A: Artifact>(&self, value: A::Value) -> Result<(), ArtifactError> {
        self.resolved
            .lock()
            .map_err(|_| ArtifactError::AccessFailed("resolved artifact mutex is poisoned".into()))?
            .set::<A>(value)
    }
}
