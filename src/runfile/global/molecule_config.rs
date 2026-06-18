use std::{num::NonZeroU8, path::PathBuf};

use toml_spanner::Toml;

use crate::molecules::units::Units;

#[derive(Debug, Toml)]
#[toml(Toml)]
pub(crate) struct MoleculeConfig {
    #[toml(
        default = default_molecule_file(),
        with = crate::runfile::validated::non_empty_path_buf
    )]
    pub(crate) geometry: PathBuf,
    #[toml(default)]
    pub(crate) charge: i32,
    #[toml(default = default_multiplicity())]
    #[toml(with = crate::runfile::validated::non_zero_u8)]
    pub(crate) multiplicity: NonZeroU8,
    #[toml(default = default_molecule_unit())]
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

    #[test]
    fn test_molecule_config_rejects_empty_geometry_path() {
        let result = toml_spanner::from_str::<MoleculeConfig>(
            r#"
            geometry = ""
            "#,
        );

        assert!(result.is_err());
    }
}
