//! CLI TOML frontend. Convert these representations to `rustiq_core::config`
//! before invoking scientific code; parsing is never needed for direct Rust use.
mod adapter;
mod diagnostics;
pub mod global;
pub mod hf;
pub mod mp2;
pub mod output;
mod units;
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

#[cfg(test)]
mod tests {
    use super::RunFile;
    use std::{fs, path::Path};

    fn collect_toml_files(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect_toml_files(&path, files);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "toml")
            {
                files.push(path);
            }
        }
    }

    #[test]
    fn test_sample_runfiles_parse_with_toml_spanner() {
        let mut files = Vec::new();
        collect_toml_files(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("samples"),
            &mut files,
        );
        files.retain(|path| {
            path.file_name()
                .is_none_or(|name| name != "invalid_diagnostics.toml")
        });
        assert!(!files.is_empty());

        for path in files {
            let content = fs::read_to_string(&path).unwrap();
            toml_spanner::from_str::<RunFile>(&content)
                .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()));
        }
    }
}
