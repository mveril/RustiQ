//! Optional TOML frontend. Convert these representations to `crate::config`
//! before invoking scientific code; parsing is never needed for direct Rust use.
mod adapter;
mod diagnostics;
pub mod global;
pub mod hf;
pub mod mp2;
pub mod output;
use global::Global;
pub mod parser;
pub mod random_config;
pub mod validated;
use toml_spanner::Toml;

#[derive(Debug, Toml)]
#[toml(Toml, recoverable)]
pub struct RunFile {
    pub global: Global,
    pub hf: Option<hf::HfConfig>,
    #[toml(default)]
    pub mp2: Option<mp2::Mp2Config>,
}

#[cfg(test)]
mod mp2_tests {
    use super::*;

    #[test]
    fn test_runfile_defaults_mp2_to_none() {
        let run: RunFile = toml_spanner::from_str(
            r#"
            [global]
            basis = "sto-3g"
            "#,
        )
        .unwrap();

        assert!(run.mp2.is_none());
    }

    #[test]
    fn test_runfile_deserializes_mp2_section() {
        let run: RunFile = toml_spanner::from_str(
            r#"
            [global]
            basis = "sto-3g"

            [mp2]
            frozen_orbitals = 1
            "#,
        )
        .unwrap();

        assert_eq!(run.mp2.unwrap().frozen_orbitals, 1);
    }
}
