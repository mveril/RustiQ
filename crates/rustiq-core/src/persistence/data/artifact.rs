use super::rustiq_data::RustiQData;
use crate::persistence::ArtifactError;

pub(crate) mod private {
    pub trait Sealed {}
}

/// A known scientific artifact. Only RustiQ's declared artifact markers implement this trait.
///
/// Values for reusable heavy artifacts must be cheap to clone: use shared immutable backing
/// storage or another representation that avoids copying the payload. The resolver may clone
/// values while moving them between the source, resolved state, and execution. Implementing
/// `Clone` as a deep copy for a large artifact is incorrect for this architecture.
pub trait Artifact: private::Sealed {
    type Value;

    /// Stable logical artifact name, independent of its storage path.
    const NAME: &'static str;

    #[doc(hidden)]
    fn is_present(data: &RustiQData) -> bool;
    #[doc(hidden)]
    fn is_compatible(
        data: &RustiQData,
        calculation: &crate::calculation::PreparedCalculation,
    ) -> bool;

    #[doc(hidden)]
    fn get(data: &mut RustiQData) -> Result<Option<&Self::Value>, ArtifactError>;
    #[doc(hidden)]
    fn set(data: &mut RustiQData, value: Self::Value) -> Result<(), ArtifactError>;
}
