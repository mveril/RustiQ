use std::{num::NonZeroU8, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::molecules::units::Units;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct MoleculeConfig {
    #[serde(default = "default_molecule_file")]
    pub(crate) geometry: PathBuf,
    #[serde(default)]
    pub(crate) charge: i32,
    #[serde(default = "default_multiplicity")]
    pub(crate) multiplicity: NonZeroU8,
    #[serde(default = "default_molecule_unit")]
    pub(crate) molecule_unit: Units,
}

impl Default for MoleculeConfig {
    fn default() -> Self {
        Self {
            geometry: default_molecule_file(),
            charge: Default::default(),
            multiplicity: default_multiplicity(),
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
    NonZeroU8::MIN
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_default_multiplicity() {
        assert_eq!(u8::from(default_multiplicity()), 1)
    }
}
