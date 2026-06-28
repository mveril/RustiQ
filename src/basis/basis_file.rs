#![allow(dead_code)]

use super::function_type::FunctionType;
use super::utils::{
    deserialize_option_from_empty_string, deserialize_vec_string_to_vec_f64,
    deserialize_vec_vec_string_to_vec_vec_f64,
};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::io::Read;

/// JSON for describing metadata for a single basis set
#[derive(Debug, Deserialize)]
pub struct BasisFile {
    #[serde(default)]
    pub(crate) auxiliaries: Auxiliaries,

    /// Brief description of the basis set
    pub(crate) description: String,

    /// Data for the elements of the basis set
    pub(crate) elements: HashMap<u32, Element>,

    /// Broad family that the basis set is from
    pub(crate) family: String,

    pub(crate) function_types: HashSet<FunctionType>,

    /// Info about which schema the file follows
    pub(crate) molssi_bse_schema: MolssiBseSchema,

    /// Canonical name for this basis set
    pub(crate) name: String,

    /// Common names/capitalization of the basis set
    pub(crate) names: Vec<String>,

    pub(crate) revision_date: String,

    pub(crate) revision_description: String,

    pub(crate) role: Role,

    pub(crate) tags: Vec<TagElement>,

    /// Version of the basis set
    pub(crate) version: String,
}

impl BasisFile {
    /// Loads a basis file from its JSON representation.
    pub fn from_reader(reader: impl Read) -> Result<Self, serde_json::Error> {
        serde_json::from_reader(reader)
    }
}

/// Auxiliary basis sets (fitting, etc) and how their role with this basis
#[derive(Debug, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AuxiliaryRole {
    Jfit,
    Jkfit,
    Rifit,
    Optri,
    Admmfit,
    Dftxfit,
    Dftjfit,
}

/// Represents either a single string or an array of unique strings
#[derive(Debug, Deserialize)]
#[serde(untagged)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AuxiliaryValue {
    Single(String),
    Multiple(Vec<String>),
}

/// Auxiliaries object with dynamic properties based on predefined role names
#[derive(Debug, Deserialize, Default)]
pub(crate) struct Auxiliaries {
    #[serde(flatten)]
    pub(crate) roles: HashMap<AuxiliaryRole, AuxiliaryValue>,
}

/// Data for a single element or atom in the basis set
#[derive(Debug, Deserialize)]
pub(crate) struct Element {
    #[serde(default)]
    pub(crate) references: Vec<Reference>,
    #[serde(default)]
    pub(crate) electron_shells: Vec<ElectronShell>,
    #[serde(default)]
    pub(crate) ecp_electrons: usize,
    /// Number of electrons replaced by ECP
    #[serde(default)]
    pub(crate) ecp_potentials: Vec<EcpPotential>,
}

/// Data for a single element or atom in the basis set
#[derive(Debug, Deserialize)]
pub(crate) struct Reference {
    /// A description of what this reference pertains to
    pub(crate) reference_description: String,
    /// Citation\/Reference keys pertaining to some basis set data
    pub(crate) reference_keys: Vec<String>,
}

/// Information for a single electronic shell
#[derive(Debug, Deserialize)]
pub(crate) struct ElectronShell {
    pub(crate) function_type: FunctionType,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_option_from_empty_string")]
    pub(crate) region: Option<ElectronShellRegion>,
    pub(crate) angular_momentum: Vec<u8>,
    #[serde(default)]
    pub(crate) r_exponents: Vec<u32>,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_vec_string_to_vec_f64")]
    pub(crate) exponents: Vec<f64>,
    #[serde(deserialize_with = "deserialize_vec_vec_string_to_vec_vec_f64")]
    pub(crate) coefficients: Vec<Vec<f64>>,
}

#[derive(Debug, Hash, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ElectronShellRegion {
    Valence,
    Polarization,
    Core,
    Tight,
    Diffuse,
}
/// ECP potential
#[derive(Debug, Deserialize)]
pub(crate) struct EcpPotential {
    pub(crate) ecp_type: EcpFunctionType,
    /// Angular momentum (as an array of integers)
    pub(crate) angular_momentum: Vec<u8>,
    /// Exponents of the r term
    pub(crate) r_exponents: Vec<u32>,
    /// Exponents of the gaussian term
    #[serde(deserialize_with = "deserialize_vec_string_to_vec_f64")]
    pub(crate) gaussian_exponents: Vec<f64>,
    /// General contraction coefficients for this contracted shell
    #[serde(deserialize_with = "deserialize_vec_string_to_vec_f64")]
    pub(crate) coefficients: Vec<f64>,
}
#[derive(Debug, Hash, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EcpFunctionType {
    ScalarEcp,
    SpinorbitEcp,
}

/// Info about which schema the file follows
#[derive(Debug, Deserialize)]
pub(crate) struct MolssiBseSchema {
    /// What type of BSE JSON file this is
    pub(crate) schema_type: SchemaType,

    /// Version of the BSE complete basis set schema being used
    pub(crate) schema_version: String,
}

/// What type of BSE JSON file this is
#[derive(Debug, Hash, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SchemaType {
    Complete,
}

/// Role that this basis plays in a calculation
#[derive(Debug, Hash, PartialEq, PartialOrd, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Role {
    Admmfit,

    Dftjfit,

    Dftxfit,

    Guess,

    Jfit,

    Jkfit,

    Optri,

    Orbital,

    Rifit,
}

/// Feature tags (for internal use/marking)
#[derive(Debug, Hash, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TagElement {
    Deprecated,
}
