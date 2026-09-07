use serde::{
    de::{Error as _, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::{
    borrow::Cow,
    fmt, io,
    path::{Component, Path},
    str::FromStr,
};
use thiserror::Error;

/// Error returned when a basis-set name cannot form a safe BSE identifier.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("invalid basis set identifier '{value}'")]
pub struct InvalidBasisId {
    value: String,
}

/// A normalized Basis Set Exchange identifier.
///
/// Identifiers are case-insensitive. BSE's filesystem representation replaces
/// `/` with `_sl_` and `*` with `_st_`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BasisId<'a>(Cow<'a, str>);

impl<'a> BasisId<'a> {
    /// Normalizes and validates a basis-set name or identifier.
    pub fn new(value: &'a str) -> Result<Self, InvalidBasisId> {
        if value.is_empty() || value.contains('\\') {
            return Err(invalid_basis_id(value));
        }

        let normalized = if value
            .chars()
            .any(|character| character.is_uppercase() || matches!(character, '/' | '*'))
        {
            Cow::Owned(
                value
                    .to_lowercase()
                    .replace('/', "_sl_")
                    .replace('*', "_st_"),
            )
        } else {
            Cow::Borrowed(value)
        };
        validate_normalized(normalized.as_ref())?;

        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Converts this identifier into an owned value.
    pub fn into_owned(self) -> BasisId<'static> {
        BasisId(Cow::Owned(self.0.into_owned()))
    }
}

impl BasisId<'static> {
    /// Normalizes a basis-set identifier while reusing an owned string when
    /// no transformation is required.
    pub fn from_string(value: String) -> Result<Self, InvalidBasisId> {
        let needs_normalization = value
            .chars()
            .any(|character| character.is_uppercase() || matches!(character, '/' | '*'));
        if needs_normalization {
            return BasisId::new(&value).map(BasisId::into_owned);
        }

        validate_normalized(&value)?;
        Ok(Self(Cow::Owned(value)))
    }
}

fn validate_normalized(value: &str) -> Result<(), InvalidBasisId> {
    if value.is_empty() || value.contains('\\') {
        return Err(invalid_basis_id(value));
    }

    let mut components = Path::new(value).components();
    let is_file_name =
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
    is_file_name
        .then_some(())
        .ok_or_else(|| invalid_basis_id(value))
}

fn invalid_basis_id(value: &str) -> InvalidBasisId {
    InvalidBasisId {
        value: value.to_owned(),
    }
}

impl From<InvalidBasisId> for io::Error {
    fn from(error: InvalidBasisId) -> Self {
        Self::new(io::ErrorKind::InvalidInput, error)
    }
}

impl AsRef<str> for BasisId<'_> {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl PartialEq<str> for BasisId<'_> {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl fmt::Display for BasisId<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for BasisId<'static> {
    type Err = InvalidBasisId;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let id: BasisId<'_> = BasisId::new(value)?;
        Ok(id.into_owned())
    }
}

impl<'a> TryFrom<&'a str> for BasisId<'a> {
    type Error = InvalidBasisId;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for BasisId<'static> {
    type Error = InvalidBasisId;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_string(value)
    }
}

impl Serialize for BasisId<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for BasisId<'de> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct BasisIdVisitor;

        impl<'de> Visitor<'de> for BasisIdVisitor {
            type Value = BasisId<'de>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a valid basis-set identifier")
            }

            fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                BasisId::new(value).map_err(E::custom)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                BasisId::new(value)
                    .map(BasisId::into_owned)
                    .map_err(E::custom)
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                BasisId::from_string(value).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(BasisIdVisitor)
    }
}

/// Deserializes an ID for a structure that must own all of its data.
pub(crate) fn deserialize_owned<'de, D>(deserializer: D) -> Result<BasisId<'static>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    BasisId::from_string(value).map_err(D::Error::custom)
}

#[cfg(feature = "online")]
#[derive(Debug, Eq, Hash, PartialEq)]
pub(crate) struct OwnedBasisId(pub BasisId<'static>);

#[cfg(feature = "online")]
impl<'de> Deserialize<'de> for OwnedBasisId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_owned(deserializer).map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_bse_names() {
        assert_eq!(BasisId::new("6-31G*").unwrap().as_str(), "6-31g_st_");
        assert_eq!(BasisId::new("cc-pV/DZ").unwrap().as_str(), "cc-pv_sl_dz");
    }

    #[test]
    fn serde_uses_the_normalized_identifier() {
        let id: BasisId<'_> = serde_json::from_str(r#""6-31G*""#).unwrap();
        assert_eq!(serde_json::to_string(&id).unwrap(), r#""6-31g_st_""#);
    }

    #[test]
    fn serde_borrows_an_already_normalized_identifier() {
        let json = r#""sto-3g""#;
        let id: BasisId<'_> = serde_json::from_str(json).unwrap();
        assert!(matches!(id.0, Cow::Borrowed("sto-3g")));
    }

    #[test]
    fn owned_deserializer_detaches_the_identifier_from_json() {
        let mut deserializer = serde_json::Deserializer::from_str(r#""sto-3g""#);
        let id = deserialize_owned(&mut deserializer).unwrap();
        assert!(matches!(id.0, Cow::Owned(_)));
    }

    #[test]
    fn owned_constructor_reuses_an_already_normalized_string() {
        let value = String::from("sto-3g");
        let allocation = value.as_ptr();
        let id = BasisId::from_string(value).unwrap();
        assert_eq!(id.as_str().as_ptr(), allocation);
    }

    #[test]
    fn borrows_an_already_normalized_identifier() {
        let id = BasisId::new("sto-3g").unwrap();
        assert!(matches!(id.0, Cow::Borrowed("sto-3g")));
    }
}
