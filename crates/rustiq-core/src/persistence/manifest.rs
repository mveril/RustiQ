use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub format_version: u32,
    pub kind: String,
    pub producer: Producer,
    pub scientific_identity: ScientificIdentityManifest,
    pub artifacts: std::collections::BTreeMap<String, ArtifactManifest>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Producer {
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScientificIdentityManifest {
    pub version: u32,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactManifest {
    pub path: String,
    pub encoding: String,
    pub dtype: String,
    pub representation: String,
    pub basis_functions: usize,
    pub shape: Vec<usize>,
    pub sha256: String,
}
