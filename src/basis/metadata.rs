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
    pub function_types: Vec<FunctionType>, // Liste de types de fonctions, toujours présente
    pub latest_version: String,            // Version représentée par une chaîne de caractères
    pub notes_exist: Vec<bool>,            // Liste de booléens indiquant si des notes existent
    pub other_names: Vec<String>, // Liste d'autres noms, peut être vide mais toujours présente
    pub relpath: String,          // Chemin relatif du fichier associé
    pub role: String,             // Rôle (par exemple orbital, jfit)
    pub tags: Vec<String>,        // Liste de tags, souvent vide mais toujours présente
    pub versions: HashMap<String, Version>, // Versions des jeux de bases
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct Version {
    pub elements: Vec<String>, // Liste des éléments pour cette version
    pub file_relpath: String,  // Chemin du fichier associé
    pub revdate: String,       // Date de révision
    pub revdesc: String,       // Description de la révision
}

impl BasisSetDetail {
    pub fn get_latest_version(&self) -> &Version {
        &self.versions[&self.latest_version]
    }
}
