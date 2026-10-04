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

    #[test]
    #[ignore = "requires Nickel; verifies the isolated POC against current Rust defaults"]
    fn nickel_poc_matches_current_runfile_defaults() {
        use std::{io::Write, process::Command};

        fn export(path: &Path) -> serde_json::Value {
            let output =
                Command::new(std::env::var_os("NICKEL_BIN").unwrap_or_else(|| "nickel".into()))
                    .args(["export", "--format", "json"])
                    .arg(path)
                    .output()
                    .expect("install Nickel or set NICKEL_BIN");
            assert!(
                output.status.success(),
                "{}: {}",
                path.display(),
                String::from_utf8_lossy(&output.stderr)
            );
            serde_json::from_slice(&output.stdout).unwrap()
        }

        fn align_with_resolved_input(value: &mut serde_json::Value) {
            if let serde_json::Value::Object(fields) = value {
                fields.retain(|_, value| !value.is_null());
                for value in fields.values_mut() {
                    align_with_resolved_input(value);
                }
            }
            if value
                .get("method")
                .and_then(|method| method.get("hf"))
                .is_some_and(serde_json::Value::is_null)
            {
                value["method"]["hf"] = serde_json::json!({
                    "method": "Auto",
                    "max_iterations": 100,
                    "convergence_threshold": 1e-8,
                    "guess": {"type": "CoreHamiltonian", "perturbation": null},
                    "diis": {"enabled": false, "max_history": 6},
                    "orthogonalization": {"linear_dependency_threshold": 1e-8},
                });
            }
        }

        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let contract = root.join("tools/nickel/calculation.ncl");
        let mut sources = Vec::new();
        collect_toml_files(&root.join("samples"), &mut sources);
        sources.retain(|path| {
            path.file_name()
                .is_none_or(|name| name != "invalid_diagnostics.toml")
        });
        for path in sources {
            let source = fs::read_to_string(&path).unwrap();
            let config: RunFile = toml_spanner::from_str(&source).unwrap();
            // Import the full TOML emitted by today's Rust frontend as a second,
            // independent oracle. This catches every injected default drifting.
            let mut full = tempfile::Builder::new().suffix(".toml").tempfile().unwrap();
            write!(full, "{}", toml_spanner::to_string(&config).unwrap()).unwrap();
            let mut program = tempfile::Builder::new().suffix(".ncl").tempfile().unwrap();
            write!(
                program,
                "let Calculation = import {} in (import {}) | Calculation",
                serde_json::to_string(&contract).unwrap(),
                serde_json::to_string(&path).unwrap()
            )
            .unwrap();
            let mut expected = export(full.path());
            align_with_resolved_input(&mut expected);
            let mut actual = export(program.path());
            // TOML cannot encode None; the normalized JSON represents it as null.
            align_with_resolved_input(&mut actual);
            assert_eq!(actual, expected, "{}", path.display());
        }
    }
}
