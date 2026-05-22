use serde::Deserialize;
use std::collections::HashMap;

use super::function_type::FunctionType;

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct BasisSetDetail {
    pub basename: String,
    pub description: String,
    pub display_name: String,
    pub family: String,
    pub function_types: Vec<FunctionType>, // List of function types, always present
    pub latest_version: String,            // Version represented as a string
    pub notes_exist: Vec<bool>,            // List of booleans indicating whether notes exist
    pub other_names: Vec<String>, // List of other names, may be empty but is always present
    pub relpath: String,          // Relative path of the associated file
    pub role: String,             // Role (for example orbital, jfit)
    pub tags: Vec<String>,        // List of tags, often empty but always present
    pub versions: HashMap<String, Version>, // Basis set versions
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct Version {
    pub elements: Vec<String>, // List of elements for this version
    pub file_relpath: String,  // Path of the associated file
    pub revdate: String,       // Revision date
    pub revdesc: String,       // Revision description
}

impl BasisSetDetail {
    pub fn get_latest_version(&self) -> &Version {
        &self.versions[&self.latest_version]
    }
}
