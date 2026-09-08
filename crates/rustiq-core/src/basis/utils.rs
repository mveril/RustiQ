use serde::de::{self, IntoDeserializer, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt;

// For fields that are directly Vec<String> to be converted to Vec<f64>
pub(crate) fn deserialize_vec_string_to_vec_f64<'de, D>(
    deserializer: D,
) -> Result<Vec<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_seq(VecStringToF64Visitor)
}

pub fn deserialize_option_from_empty_string<'de, D, T>(
    deserializer: D,
) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    match opt.as_deref() {
        Some("") => Ok(None),
        Some(s) => T::deserialize(s.into_deserializer()).map(Some),
        None => Ok(None),
    }
}

struct VecStringToF64Visitor;

impl<'de> Visitor<'de> for VecStringToF64Visitor {
    type Value = Vec<f64>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a sequence of strings that can be parsed into f64")
    }

    fn visit_seq<S>(self, mut seq: S) -> Result<Vec<f64>, S::Error>
    where
        S: SeqAccess<'de>,
    {
        let mut vec = Vec::new();

        while let Some(elem) = seq.next_element::<String>()? {
            let parsed_elem = elem.parse::<f64>().map_err(de::Error::custom)?;
            vec.push(parsed_elem);
        }

        Ok(vec)
    }
}

// For fields that are Vec<Vec<String>> to be converted to Vec<Vec<f64>>
pub(crate) fn deserialize_vec_vec_string_to_vec_vec_f64<'de, D>(
    deserializer: D,
) -> Result<Vec<Vec<f64>>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_seq(VecVecStringToF64Visitor)
}

struct VecVecStringToF64Visitor;

impl<'de> Visitor<'de> for VecVecStringToF64Visitor {
    type Value = Vec<Vec<f64>>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a sequence of sequences of strings that can be parsed into f64")
    }

    fn visit_seq<S>(self, mut seq: S) -> Result<Vec<Vec<f64>>, S::Error>
    where
        S: SeqAccess<'de>,
    {
        let mut vec = Vec::new();

        while let Some(sub_vec) = seq.next_element::<Vec<String>>()? {
            let parsed_sub_vec = sub_vec
                .into_iter()
                .map(|elem| elem.parse::<f64>().map_err(de::Error::custom))
                .collect::<Result<Vec<f64>, _>>()?;
            vec.push(parsed_sub_vec);
        }

        Ok(vec)
    }
}
