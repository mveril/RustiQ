use super::rustiq_data::RustiQData;
use crate::persistence::PersistenceError;

pub(crate) mod private {
    pub trait Sealed {}
}

/// A known scientific artifact. Only RustiQ's declared artifact markers implement this trait.
pub trait Artifact: private::Sealed {
    type Value;

    #[doc(hidden)]
    fn get(data: &mut RustiQData) -> Result<Option<&Self::Value>, PersistenceError>;
    #[doc(hidden)]
    fn set(data: &mut RustiQData, value: Self::Value) -> Result<(), PersistenceError>;
}
