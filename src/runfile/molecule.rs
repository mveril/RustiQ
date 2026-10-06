use std::{num::NonZeroU8, path::PathBuf};

use rustiq_core::molecules::units::Units;

#[derive(Debug, serde::Serialize)]
pub struct MoleculeConfig {
    pub geometry: PathBuf,
    pub charge: i32,
    pub multiplicity: NonZeroU8,
    #[serde(serialize_with = "crate::runfile::units::serialize")]
    pub units: Units,
}

impl Default for MoleculeConfig {
    fn default() -> Self {
        Self {
            geometry: default_molecule_file(),
            charge: Default::default(),
            multiplicity: default_multiplicity(),
            units: default_units(),
        }
    }
}

fn default_molecule_file() -> PathBuf {
    "./molecule.xyz".into()
}

fn default_units() -> Units {
    Units::Angstrom
}

fn default_multiplicity() -> NonZeroU8 {
    NonZeroU8::MIN
}
