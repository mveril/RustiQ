//! CLI TOML frontend. Convert these representations to `rustiq_core::config`
//! before invoking scientific code; parsing is never needed for direct Rust use.
mod adapter;
pub mod basis;
pub mod cache;
mod diagnostics;
pub mod hf;
pub mod integrals;
pub mod method;
pub mod molecule;
pub mod mp2;
pub mod output;
pub mod parser;
pub mod random_config;
mod units;
pub mod validated;

use basis::BasisConfig;
use cache::CacheConfig;
use integrals::IntegralConfig;
use method::MethodConfig;
use molecule::MoleculeConfig;
use output::OutputConfig;
use toml_spanner::Toml;

#[derive(Debug, Toml)]
#[toml(Toml, recoverable)]
pub struct RunFile {
    #[toml(default, style = Header)]
    pub molecule: MoleculeConfig,
    #[toml(style = Header)]
    pub basis: BasisConfig,
    #[toml(default, style = Implicit)]
    pub method: MethodConfig,
    #[toml(default, style = Header)]
    pub integrals: IntegralConfig,
    #[toml(default, style = Header)]
    pub cache: CacheConfig,
    #[toml(default, style = Header)]
    pub output: OutputConfig,
}

#[cfg(test)]
#[allow(
    unknown_lints,
    reason = "assert_is_empty is only available starting with Clippy 1.99"
)]
#[allow(
    clippy::assert_is_empty,
    reason = "Idiomatic is_empty assertions express the test intent without typed empty collections"
)]
mod tests {
    use super::*;
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
    fn sample_runfiles_parse() {
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
