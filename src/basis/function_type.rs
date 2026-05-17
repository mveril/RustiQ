use serde::Deserialize;

/// A list of function types in a basis set
#[derive(Debug, Hash, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FunctionType {
    Gto,
    GtoCartesian,
    GtoSpherical,
    ScalarEcp,
    SpinorbitEcp,
    Sto,
}
