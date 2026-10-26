use serde::Deserialize;

/// A list of function types in a basis set
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FunctionType {
    Gto,
    GtoCartesian,
    GtoSpherical,
    ScalarEcp,
    SpinorbitEcp,
    Sto,
}
