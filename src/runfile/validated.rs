pub use rustiq_core::config::validated::{DiisSize, NonNegativeFiniteF64, PositiveFiniteF64};

macro_rules! serialize_inner {
    ($module:ident, $ty:ty) => {
        pub(crate) mod $module {
            pub(crate) fn serialize<S: serde::Serializer>(
                value: &$ty,
                serializer: S,
            ) -> Result<S::Ok, S::Error> {
                serde::Serialize::serialize(&value.into_inner(), serializer)
            }
        }
    };
}
serialize_inner!(positive_finite_f64, super::PositiveFiniteF64);
serialize_inner!(non_negative_finite_f64, super::NonNegativeFiniteF64);
serialize_inner!(diis_size, super::DiisSize);

pub(crate) mod optional_positive_finite_f64 {
    #[allow(
        clippy::ref_option,
        reason = "Serde serializer callbacks receive references"
    )]
    pub(crate) fn serialize<S: serde::Serializer>(
        value: &Option<super::PositiveFiniteF64>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(value.map_or(0.0, super::PositiveFiniteF64::into_inner))
    }
}
