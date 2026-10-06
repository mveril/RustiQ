use std::{num::NonZeroU8, path::PathBuf};

use toml_spanner::Toml;

use rustiq_core::molecules::units::Units;

#[derive(Debug, Toml)]
#[toml(Toml)]
pub struct MoleculeConfig {
    #[toml(
        default = default_molecule_file(),
        with = crate::runfile::validated::non_empty_path_buf
    )]
    pub geometry: PathBuf,
    #[toml(default)]
    pub charge: i32,
    #[toml(default = default_multiplicity())]
    #[toml(with = crate::runfile::validated::non_zero_u8)]
    pub multiplicity: NonZeroU8,
    #[toml(default = default_units())]
    #[toml(with = crate::runfile::units)]
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
