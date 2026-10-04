//! Deserialize fully evaluated Nickel JSON without teaching the CLI Nickel.
use serde::{Deserialize, Serialize};
use std::{io::Read, num::NonZeroU8, path::PathBuf};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ResolvedCalculationConfig {
    molecule: Molecule,
    basis: Basis,
    method: Method,
    integrals: Integrals,
    cache: Cache,
    output: Output,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Molecule {
    geometry: PathBuf,
    charge: i32,
    multiplicity: NonZeroU8,
    units: Units,
}

#[derive(Debug, Deserialize, Serialize)]
enum Units {
    Angstrom,
    Bohr,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Basis {
    name: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Method {
    hf: Hf,
    mp2: Option<Mp2>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Hf {
    method: HfMethod,
    max_iterations: std::num::NonZeroUsize,
    convergence_threshold: f64,
    guess: Guess,
    diis: Diis,
    orthogonalization: Orthogonalization,
}

#[derive(Debug, Deserialize, Serialize)]
enum HfMethod {
    Auto,
    Rhf,
    Uhf,
}

#[derive(Debug, Deserialize, Serialize)]
// Serde does not support deny_unknown_fields alongside flatten. Nickel checks
// the closed guess records before they cross this boundary.
#[serde(tag = "type")]
enum Guess {
    CoreHamiltonian {
        perturbation: Option<Distribution>,
    },
    OneElectron {
        perturbation: Option<Distribution>,
    },
    Random {
        #[serde(flatten)]
        distribution: Distribution,
    },
    Zero,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "distribution", deny_unknown_fields)]
enum Distribution {
    Normal {
        mean: f64,
        std_dev: f64,
        seed: Option<u64>,
    },
    Uniform {
        min: f64,
        max: f64,
        seed: Option<u64>,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Diis {
    enabled: bool,
    max_history: usize,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Orthogonalization {
    linear_dependency_threshold: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Mp2 {
    frozen_orbitals: usize,
    memory_limit: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Integrals {
    schwarz_threshold: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Cache {
    enabled: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Output {
    scf: ScfOutput,
}

#[derive(Debug, Deserialize, Serialize)]
enum ScfOutput {
    Normal,
    Quiet,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ResolvedInput {
    calculations: Vec<ResolvedCalculationConfig>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut json = String::new();
    std::io::stdin().read_to_string(&mut json)?;
    let resolved: ResolvedInput = serde_json::from_str(&json)?;
    println!("{}", serde_json::to_string_pretty(&resolved)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::Path, process::Command};

    fn export(path: &Path) -> std::process::Output {
        Command::new(std::env::var_os("NICKEL_BIN").unwrap_or_else(|| "nickel".into()))
            .args(["export", "--format", "json"])
            .arg(path)
            .output()
            .expect("install Nickel or set NICKEL_BIN")
    }

    fn assert_equivalent(left: &serde_json::Value, right: &serde_json::Value) {
        use serde_json::Value;
        match (left, right) {
            (Value::Number(a), Value::Number(b)) if a.is_f64() || b.is_f64() => {
                // Nickel writes integral floats as integers; the DTO writes f64.
                assert_eq!(a.as_f64(), b.as_f64());
            }
            (Value::Object(a), Value::Object(b)) => {
                assert_eq!(a.len(), b.len());
                for (key, value) in a {
                    assert_equivalent(value, b.get(key).expect("same JSON keys"));
                }
            }
            (Value::Array(a), Value::Array(b)) => {
                assert_eq!(a.len(), b.len());
                for (left, right) in a.iter().zip(b) {
                    assert_equivalent(left, right);
                }
            }
            _ => assert_eq!(left, right),
        }
    }

    #[test]
    #[ignore = "requires the Nickel CLI; run explicitly as documented"]
    fn nickel_exports_deserialize_and_invalid_inputs_fail() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/nickel");
        for name in [
            "import-toml.ncl",
            "single.ncl",
            "multiple.ncl",
            "variants.ncl",
        ] {
            let output = export(&root.join(name));
            assert!(
                output.status.success(),
                "{name}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let dto: ResolvedInput = serde_json::from_slice(&output.stdout)
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            let roundtrip = serde_json::to_value(&dto).unwrap();
            assert_equivalent(&roundtrip, &json);
            if name == "multiple.ncl" {
                assert_eq!(json["calculations"].as_array().unwrap().len(), 3);
            }
            if name == "import-toml.ncl" {
                assert_eq!(json["calculations"].as_array().unwrap().len(), 1);
                assert_eq!(
                    json["calculations"][0]["molecule"]["geometry"],
                    "../molecule.xyz"
                );
                assert_eq!(
                    json["calculations"][0]["method"]["hf"]["diis"]["max_history"],
                    6
                );
                assert_eq!(
                    json["calculations"][0]["method"]["hf"]["guess"]["type"],
                    "CoreHamiltonian"
                );
                assert!(json["calculations"][0]["method"]["mp2"].is_null());
            }
        }
        for entry in std::fs::read_dir(root.join("invalid")).unwrap() {
            let path = entry.unwrap().path();
            let output = export(&path);
            assert!(
                !output.status.success(),
                "{} unexpectedly succeeded",
                path.display()
            );
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains("contract") || stderr.contains("missing definition"),
                "{stderr}"
            );
        }
    }

    fn evaluate_in_process(path: &Path) -> Result<ResolvedInput, String> {
        let source = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        let mut context =
            nickel_lang::Context::new().with_source_name(path.to_string_lossy().into_owned());
        let expr = context
            .eval_deep(&source)
            .map_err(|error| format_nickel_error(&error))?;
        let json = context
            .expr_to_json(&expr)
            .map_err(|error| format_nickel_error(&error))?;
        serde_json::from_str(&json).map_err(|error| error.to_string())
    }

    fn format_nickel_error(error: &nickel_lang::Error) -> String {
        let mut diagnostic = Vec::new();
        error
            .format(&mut diagnostic, nickel_lang::ErrorFormat::Text)
            .expect("format Nickel diagnostic");
        String::from_utf8_lossy(&diagnostic).into_owned()
    }

    #[test]
    fn nickel_library_evaluates_contract_defaults_and_reports_errors() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/nickel");
        let resolved = evaluate_in_process(&root.join("single.ncl")).unwrap();
        assert_eq!(resolved.calculations.len(), 1);
        let calculation = &resolved.calculations[0];
        assert_eq!(calculation.basis.name, "sto-3g");
        assert_eq!(calculation.molecule.multiplicity.get(), 1);
        assert_eq!(calculation.method.hf.diis.max_history, 6);

        let error = evaluate_in_process(&root.join("invalid/zero-perturbation.ncl"))
            .expect_err("Zero must reject perturbation");
        assert!(error.contains("contract"), "{error}");
        assert!(error.contains("zero-perturbation.ncl"), "{error}");
    }
}
