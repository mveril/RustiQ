use std::{num::NonZeroU8, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::molecules::units::Units;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct MoleculeConfig {
    #[serde(default = "default_molecule_file")]
    pub(crate) geometry: PathBuf,
    #[serde(default)]
    pub(crate) charge: i8,
    #[serde(default = "default_multiplicity")]
    pub(crate) mulitplicity: NonZeroU8,
    #[serde(default = "default_molecule_unit")]
    pub(crate) molecule_unit: Units,
}

impl Default for MoleculeConfig {
    fn default() -> Self {
        Self {
            geometry: default_molecule_file(),
            charge: Default::default(),
            mulitplicity: default_multiplicity(),
            molecule_unit: default_molecule_unit(),
        }
    }
}

fn default_molecule_file() -> PathBuf {
    "./molecule.xyz".into()
}

fn default_molecule_unit() -> Units {
    Units::Angstrom
}

fn default_multiplicity() -> NonZeroU8 {
    unsafe { NonZeroU8::new_unchecked(1) }
}
